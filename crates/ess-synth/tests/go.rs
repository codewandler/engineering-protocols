//! The billing specification, emitted for the second target, from the files it actually lives in.
//!
//! W7.3's acceptance criteria, executed rather than asserted: the plan does not change to admit a
//! second emitter, the Go module encodes every construct honestly or refuses it out loud, and
//! emitting twice is byte-identical.
//!
//! # What is checked here, and what is checked in the gate
//!
//! These tests read bytes. Whether the emitted module *compiles*, *vets* and is already
//! `gofmt`-clean is checked by `cargo xtask synth`, which owns the committed tree and fails
//! loudly when the Go toolchain is missing — the same division the Rust target already uses, where
//! `cargo check` inside the generated workspace is a gate step rather than a unit test. A test
//! suite that shelled out to `go` would make `cargo test` depend on a toolchain it has no other
//! reason to need.

use std::path::{Path, PathBuf};

use ess_compiler::ir::EssIr;
use ess_compiler::resolve::compile_locating;
use ess_compiler::source::SourceMap;
use ess_domain::spec::{RawSpecFile, Specification};
use ess_domain::system::Source;
use ess_synth::{synthesize, synthesize_for, CapabilityKind, Synthesis, Target};

/// The example directory.
fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/billing")
        .canonicalize()
        .expect("the billing example exists")
}

/// Every `.yaml` file in the example, relative to it, in a stable order.
fn files() -> Vec<String> {
    let base = example();
    let mut found = Vec::new();
    let mut pending = vec![base.clone()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("the example is readable") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|it| it == "yaml") {
                found.push(
                    path.strip_prefix(&base)
                        .expect("inside the example")
                        .display()
                        .to_string(),
                );
            }
        }
    }
    found.sort();
    found
}

/// The billing example, compiled where it lives.
fn billing() -> EssIr {
    let labels = files();
    let mut sources = SourceMap::new();
    let mut parsed = Vec::new();
    for label in &labels {
        let text = std::fs::read_to_string(example().join(label))
            .unwrap_or_else(|error| panic!("{label} is readable: {error}"));
        let raw = RawSpecFile::parse(&text)
            .unwrap_or_else(|error| panic!("{label} is well formed: {error}"));
        sources.insert(label.clone(), text);
        parsed.push((Source::new(label.clone()), raw));
    }
    let specification = Specification::assemble(parsed)
        .unwrap_or_else(|errors| panic!("the billing specification validates:\n{errors}"));
    compile_locating(&specification, &sources, &labels)
        .unwrap_or_else(|diagnostics| panic!("the billing specification resolves:\n{diagnostics}"))
}

/// An inline fixture, assembled and compiled from YAML strings.
fn fixture(documents: &[(&str, &str)]) -> EssIr {
    let mut sources = SourceMap::new();
    let mut labels = Vec::new();
    let mut parsed = Vec::new();
    for (label, text) in documents {
        let raw = RawSpecFile::parse(text)
            .unwrap_or_else(|error| panic!("the fixture `{label}` is well formed: {error}"));
        sources.insert((*label).to_owned(), (*text).to_owned());
        labels.push((*label).to_owned());
        parsed.push((Source::new((*label).to_owned()), raw));
    }
    let specification = Specification::assemble(parsed)
        .unwrap_or_else(|errors| panic!("the fixture validates:\n{errors}"));
    compile_locating(&specification, &sources, &labels)
        .unwrap_or_else(|diagnostics| panic!("the fixture resolves:\n{diagnostics}"))
}

/// `true` for a generated Go source file.
///
/// Its own predicate because every path here is one this emitter chose, so the extension is
/// exactly what the emitter wrote and never a filesystem's idea of it.
fn is_go(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .is_some_and(|it| it == "go")
}

/// `true` for one of the two canonical-JSON documents.
fn is_json(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .is_some_and(|it| it == "json")
}

/// The Go module the billing specification determines.
fn go() -> Synthesis {
    synthesize_for(&billing(), Target::Go)
}

/// One generated artifact's contents, by path.
fn artifact(synthesis: &Synthesis, path: &str) -> String {
    synthesis
        .artifacts
        .get(path)
        .unwrap_or_else(|| {
            panic!(
                "no artifact at `{path}`; the synthesis wrote {:?}",
                synthesis.artifacts.keys().collect::<Vec<_>>()
            )
        })
        .contents
        .clone()
}

// ---- the seam ------------------------------------------------------------------------------------

#[test]
fn the_plan_is_byte_identical_in_both_targets_trees() {
    // W7.3's central claim, and the cheapest possible test of it: if the plan had to change to
    // admit a second emitter, these bytes would differ. They are the *same document*, rendered
    // from the same planner, and each target's tree carries a copy.
    let rust = synthesize(&billing());
    let go = go();
    for plan in [ess_synth::PLAN_MARKDOWN, ess_synth::PLAN_JSON] {
        assert_eq!(
            artifact(&rust, plan),
            artifact(&go, plan),
            "`{plan}` differs between the Rust and Go trees, so the plan is not language-neutral"
        );
    }
    assert_eq!(
        rust.plan, go.plan,
        "the two targets planned the same specification differently"
    );
}

#[test]
fn the_rust_target_reports_nothing_and_the_go_target_reports_its_weakenings() {
    // The asymmetry is the finding, not a defect: Rust carried the plan whole, Go did not, and
    // "did not" is a document rather than an absence.
    assert!(
        synthesize(&billing()).target.is_none(),
        "the first target carried the plan whole, so it has no target report to write"
    );
    let report = go().target.expect("the Go target writes a report");
    assert_eq!(report.target, "go");
    assert!(
        !report.weakenings.is_empty(),
        "a target that claims to have weakened nothing has stopped looking"
    );
    for weakening in &report.weakenings {
        assert!(
            !weakening.affects.is_empty(),
            "a weakening that touches no capability kind is not answerable from the parity table"
        );
    }
}

#[test]
fn every_weakening_is_visible_in_the_generated_source_and_not_only_in_the_report() {
    // A weakening recorded only beside the code is a weakening the next reader of the code does
    // not meet. Every generated sealed interface says so where it is declared.
    let synthesis = go();
    let invoice = artifact(&synthesis, "types/invoice/invoice.go");
    assert!(
        invoice.contains("Go cannot check that a `switch` over it handles every case"),
        "the exhaustiveness weakening has to be stated in the source, not only in TARGET.md"
    );
    assert!(
        invoice.contains("recorded in TARGET.md"),
        "and it has to point at the document that carries the whole list"
    );
    let target = artifact(&synthesis, ess_synth::TARGET_MARKDOWN);
    assert!(
        target.contains("byte-identical in every target's tree"),
        "the target report says what it is *not* about: the plan"
    );
    assert!(
        target.contains("protocol ess synthesize --target go"),
        "and names the command that rewrites it: {target}"
    );
}

// ---- the encodings -------------------------------------------------------------------------------

#[test]
fn a_closed_set_is_sealed_by_an_unexported_marker_so_no_other_package_can_join_it() {
    // The encoding the whole target rests on. `Payee` is the tagged union, `Channel` the enum and
    // `CreateInvoiceOutcome` the command's outcome set — one construct in Go, because Go has one
    // way to say "one of these".
    let invoice = artifact(&go(), "types/invoice/invoice.go");
    for (interface, marker, variant) in [
        ("Payee", "isPayee", "PayeeCompany"),
        ("Channel", "isChannel", "ChannelEmail"),
        (
            "CreateInvoiceOutcome",
            "isCreateInvoiceOutcome",
            "CreateInvoiceOutcomeAccepted",
        ),
    ] {
        assert!(
            invoice.contains(&format!("type {interface} interface {{\n\t{marker}()\n}}")),
            "`{interface}` is not sealed by an unexported marker method"
        );
        assert!(
            invoice.contains(&format!("func ({variant}) {marker}() {{}}")),
            "`{variant}` does not join `{interface}`"
        );
        assert!(
            marker.chars().next().is_some_and(char::is_lowercase),
            "an exported marker is implementable from any package, which is not a closed set"
        );
    }
}

#[test]
fn a_newtype_is_a_guarded_struct_and_never_a_defined_string() {
    // `type Email string` is the encoding a Go author reaches for first, and it gives the
    // guarantee away: an untyped constant assigns straight into it. The unexported field is what
    // makes the wrapper a wrapper.
    let invoice = artifact(&go(), "types/invoice/invoice.go");
    assert!(
        invoice.contains("type Email struct {\n\tvalue string\n}"),
        "the newtype has to guard its representation behind an unexported field"
    );
    assert!(
        !invoice.contains("type Email string"),
        "a defined string type accepts `var e Email = \"anything\"`, which is exactly the \
         distinctness the declaration exists for"
    );
    assert!(
        invoice.contains("func NewEmail(value string) Email {"),
        "and the constructor is the only door to a populated value"
    );
    assert!(
        invoice.contains("func (v Email) Value() string {"),
        "with an accessor, because another package's conversion has to read the representation"
    );
}

#[test]
fn an_illegal_transition_is_a_method_that_does_not_exist() {
    // The Rust emitter refuses an undeclared move with typestate; Go cannot carry a state in a
    // type parameter and *can* carry one per state. The guarantee is the same and this is what
    // makes it so: a move is a method on exactly the states that declare it.
    let invoice = artifact(&go(), "types/invoice/invoice.go");
    for (state, legal, illegal) in [
        ("InvoiceInDraft", vec!["Issue", "Cancel"], vec!["Settle"]),
        ("InvoiceInIssued", vec!["Settle", "Cancel"], vec!["Issue"]),
        ("InvoiceInPaid", vec![], vec!["Issue", "Settle", "Cancel"]),
        (
            "InvoiceInCancelled",
            vec![],
            vec!["Issue", "Settle", "Cancel"],
        ),
    ] {
        for method in legal {
            assert!(
                invoice.contains(&format!("func (v {state}) {method}() ")),
                "`{state}` declares `{method}` and the emitted API does not have it"
            );
        }
        for method in illegal {
            assert!(
                !invoice.contains(&format!("func (v {state}) {method}() ")),
                "`{state}` has a `{method}` method the specification does not declare, so an \
                 illegal move compiles"
            );
        }
    }
    assert!(
        invoice.contains("func NewInvoice(data InvoiceData) InvoiceInDraft {"),
        "the one constructor rests in the initial state"
    );
    for state in ["InvoiceInIssued", "InvoiceInPaid", "InvoiceInCancelled"] {
        assert!(
            !invoice.contains(&format!(") {state} {{\n\treturn {state}{{data: data}}\n}}")),
            "`{state}` has a constructor, so the lifecycle can be entered part-way through"
        );
    }
}

#[test]
fn refinement_answers_ok_because_a_sealed_interfaces_zero_value_names_no_state() {
    // Rust's `refine` is total. Go's cannot be, and the emitted signature says so rather than
    // pretending — the weakening is in the report *and* in the API a caller has to use.
    let invoice = artifact(&go(), "types/invoice/invoice.go");
    assert!(
        invoice.contains("func (v InvoiceSnapshot) Refine() (AnyInvoice, bool) {"),
        "refinement has to admit the case Go's zero value creates"
    );
    assert!(
        invoice.contains("Rust's is total, and this one cannot be"),
        "and the doc comment has to say which target it is weaker than"
    );
    for state in ["Draft", "Issued", "Paid", "Cancelled"] {
        assert!(
            invoice.contains(&format!(
                "\tcase InvoiceState{state}:\n\t\treturn InvoiceIn{state}{{data: v.Data}}, true"
            )),
            "`{state}` has no arm, so a snapshot in it refines to nothing"
        );
    }
}

#[test]
fn a_command_outcome_keeps_the_refusal_beside_the_success() {
    let email = artifact(&go(), "types/email/email.go");
    assert!(
        email.contains("func (SendEmailOutcomeSent) isSendEmailOutcome() {}")
            && email.contains("func (SendEmailOutcomeFailed) isSendEmailOutcome() {}"),
        "both branches join the outcome set, or a consumer handles only the happy path"
    );
    assert!(
        email.contains("\tError Undeliverable"),
        "the refusing branch carries the declared error, not a stringly one"
    );
}

#[test]
fn an_obligation_is_an_interface_and_a_stub_that_returns_a_value_never_a_panic() {
    let synthesis = go();
    let email = artifact(&synthesis, "types/email/email.go");
    assert!(
        email.contains("type SendEmailBehavior interface {"),
        "an owed behaviour is a seam, not a hole"
    );
    assert!(
        email.contains(
            "func (Unimplemented) SendEmail(input SendEmail) (SendEmailOutcome, \
             *obligation.UnmetObligation) {"
        ),
        "and the shared stub satisfies it"
    );
    for emitted in synthesis.artifacts.values() {
        if !is_go(&emitted.path) {
            continue;
        }
        for banned in ["panic(", "todo(", "os.Exit"] {
            assert!(
                !emitted.contents.contains(banned),
                "`{}` reaches for `{banned}`; an unmet obligation is a value, so that a module \
                 built on stubs runs and reports its own gaps",
                emitted.path
            );
        }
    }
}

#[test]
fn the_plans_obligations_and_the_modules_stubs_are_the_same_list() {
    // The bijection wave 6.2 asks for, read back out of the generated sources by their one
    // construction site rather than from any list the emitter keeps.
    let synthesis = go();
    let mut stubs: Vec<(String, String)> = Vec::new();
    for emitted in synthesis.artifacts.values() {
        let text = &emitted.contents;
        let marker = "UnmetObligation{Capability: \"";
        let mut from = 0;
        while let Some(position) = text[from..].find(marker) {
            let at = from + position + marker.len();
            let capability_end = text[at..].find('"').expect("the capability closes") + at;
            let source_at = text[capability_end..]
                .find("Source: \"")
                .expect("the source follows")
                + capability_end
                + "Source: \"".len();
            let source_end = text[source_at..].find('"').expect("the source closes") + source_at;
            stubs.push((
                text[at..capability_end].to_owned(),
                text[source_at..source_end].to_owned(),
            ));
            from = source_end;
        }
    }
    stubs.sort();

    let mut owed: Vec<(String, String)> = synthesis
        .plan
        .obligations()
        .map(|(capability, _)| {
            (
                capability.kind.describes().to_owned(),
                capability.source.clone(),
            )
        })
        .collect();
    owed.sort();
    assert_eq!(
        stubs, owed,
        "every obligation is visible twice — in the plan, and as a typed refusal in the module"
    );
}

#[test]
fn the_transport_is_the_one_the_billing_binding_requires() {
    let system = artifact(&go(), "system/system.go");
    for expected in [
        // the log, the cursor, and the pump that delivers until quiescent
        "func (s *System) Pump() *obligation.UnmetObligation {",
        "func (s *System) collect() {",
        "func (s *System) deliver(event SystemEvent) *obligation.UnmetObligation {",
        // at-least-once means a second delivery of one occurrence is legal and available
        "func (s *System) Redeliver(event SystemEvent) *obligation.UnmetObligation {",
        // the declared failure policy runs on the declared refusal, and escalation is owed
        "case email.SendEmailOutcomeFailed:",
        "escalation, owed := s.obligations.NotifyOnInvoiceCreatedEscalation(input)",
        "s.published = append(s.published, SystemEventDeliveryEscalated{Event: escalation})",
        // and what the binding invoked is observable
        "s.invocations = append(s.invocations, BindingInvocationNotifyOnInvoiceCreated{Input: input})",
    ] {
        assert!(
            system.contains(expected),
            "the generated transport is missing `{expected}`:\n{system}"
        );
    }
    assert!(
        !system.contains("case email.SendEmailOutcomeFailed:\n\t\treturn"),
        "an unmet obligation must not be routed into the failure policy — that would publish a \
         domain event no domain fact caused"
    );
}

#[test]
fn the_generated_transformation_reads_the_event_through_the_declared_crossing() {
    let system = artifact(&go(), "system/system.go");
    assert!(
        system
            .contains("Recipient: email.EmailAddressFromBillingInvoiceEmail(event.CustomerEmail)"),
        "the mapped input crosses by the generated conversion, not by re-wrapping a string:\n\
         {system}"
    );
    assert!(
        system.contains("Template: email.NewTemplateId(\"invoice-created\")"),
        "and a literal is wrapped by the target type's constructor"
    );
}

/// A specification whose crossing is declared but not mechanical, whose transformation is
/// therefore owed, and whose binding retries.
///
/// The billing example exercises none of these three: its one crossing is mechanical, its one
/// transformation is generated, and its one binding escalates. Without this fixture three emitted
/// shapes would ship untested.
fn relay() -> EssIr {
    fixture(&[
        (
            "system.yaml",
            "format: ess/1\nsystem: relay\nversion: v1\ndomains:\n  - relay.core\n",
        ),
        (
            "core.yaml",
            "domain: relay.core\ntypes:\n  - name: relay.core.AccountId\n    kind: newtype\n    \
             of: Uuid\n  - name: relay.core.AccountRef\n    kind: newtype\n    of: \
             String\nconversions:\n  - from: relay.core.AccountId\n    to: \
             relay.core.AccountRef\n    because: an account may be referred to by its id rendered \
             as text.\nevents:\n  - name: relay.core.Fired\n    fields:\n      - name: \
             account\n        type: relay.core.AccountId\n  - name: relay.core.Handled\n    \
             fields:\n      - name: account\n        type: \
             relay.core.AccountRef\ncommands:\n  - name: relay.core.Handle\n    input:\n      - \
             name: account\n        type: relay.core.AccountRef\n    outcomes:\n      - name: \
             done\n        emits:\n          - relay.core.Handled\n      - name: rejected\n        \
             external: the account is closed downstream\n        error: \
             relay.core.Rejected\nerrors:\n  - name: relay.core.Rejected\n    summary: The \
             account was not accepted.\n",
        ),
        (
            "wiring.yaml",
            "components:\n  - component: relay-service\n    owns:\n      domains:\n        - \
             relay.core\n    accepts:\n      commands:\n        - relay.core.Handle\n    \
             publishes:\n      events:\n        - relay.core.Handled\nbindings:\n  - id: \
             relay-on-fired\n    when:\n      event: relay.core.Fired\n    invoke:\n      \
             command: relay.core.Handle\n    mapping:\n      account: event.account\n    \
             delivery: at_least_once\n    on_failure: retry\n",
        ),
    ])
}

#[test]
fn an_owed_crossing_gets_its_own_package_because_go_refuses_an_import_cycle() {
    // Rust files an owed conversion beside the refusal type; Go cannot, because a bounded
    // context's package must import that type and an owed crossing names both contexts. The file
    // moves; the plan entry does not.
    let synthesis = synthesize_for(&relay(), Target::Go);
    let conversion = artifact(&synthesis, "types/conversion/conversion.go");
    assert!(
        conversion.contains("type RelayCoreAccountIdToRelayCoreAccountRefConversion interface {"),
        "the owed crossing is a seam:\n{conversion}"
    );
    assert!(
        conversion.contains(
            "ConvertRelayCoreAccountIdToRelayCoreAccountRef(value core.AccountId) (core.AccountRef, *obligation.UnmetObligation)"
        ),
        "the method carries both ends, because Go gives a type one method set and one shared stub \
         cannot answer two seams that both call their method `Convert`:\n{conversion}"
    );
    assert!(
        !artifact(&synthesis, "types/core/core.go").contains("Conversion interface"),
        "the seam must not also be declared where either end lives"
    );
}

#[test]
fn an_owed_transformation_and_a_retry_policy_are_emitted_the_way_the_binding_declares_them() {
    let system = artifact(&synthesize_for(&relay(), Target::Go), "system/system.go");
    assert!(
        system.contains("input, unmet := s.obligations.RelayOnFiredInput(event)"),
        "an undetermined mapping is routed through the owed seam, never guessed:\n{system}"
    );
    assert!(
        !system.contains("func RelayOnFired(event"),
        "and no transformation is generated beside it"
    );
    assert!(
        system.contains("s.retries = append(s.retries, SystemEventFired{Event: event})"),
        "`on_failure: retry` holds the event for the next pump:\n{system}"
    );
    assert!(
        system.contains("retrying := s.retries")
            && system.contains("for _, held := range retrying {"),
        "and the next pump makes one more attempt, which is the redelivery this transport provides"
    );
}

// ---- what this target refuses --------------------------------------------------------------------

/// A specification whose command input reaches a map keyed by opaque bytes.
fn bytes_keyed() -> EssIr {
    fixture(&[
        (
            "system.yaml",
            "format: ess/1\nsystem: probe\nversion: v1\ndomains:\n  - probe.core\n",
        ),
        (
            "core.yaml",
            "domain: probe.core\ntypes:\n  - name: probe.core.Attachments\n    kind: struct\n    \
             fields:\n      - name: parts\n        type: \"Map<Bytes, String>\"\nevents:\n  - \
             name: probe.core.Attached\n    fields:\n      - name: note\n        type: \
             String\ncommands:\n  - name: probe.core.Attach\n    input:\n      - name: \
             attachments\n        type: probe.core.Attachments\n    outcomes:\n      - name: \
             done\n        emits:\n          - probe.core.Attached\n",
        ),
    ])
}

#[test]
fn a_map_keyed_by_bytes_is_refused_at_the_target_stage_and_never_emitted() {
    // The one thing Go cannot spell, and the reason a second target was worth building: Rust's
    // `BTreeMap<Vec<u8>, V>` is ordinary, and a Go map key must be comparable.
    let synthesis = synthesize_for(&bytes_keyed(), Target::Go);
    let report = synthesis
        .target
        .as_ref()
        .expect("the Go target writes a report");
    let refused: Vec<&str> = report
        .refusals
        .iter()
        .map(|refusal| refusal.capability.source.as_str())
        .collect();
    assert!(
        refused.contains(&"probe.core.Attachments"),
        "the unrepresentable type is not refused: {refused:?}"
    );
    assert!(
        refused.contains(&"probe.core.Attach"),
        "and the refusal has to travel the way dependence does: {refused:?}"
    );
    let detail = &report
        .refusals
        .iter()
        .find(|refusal| refusal.capability.source == "probe.core.Attach")
        .expect("the command is refused")
        .detail;
    assert!(
        detail.contains("a Go map key must be comparable"),
        "the refusal names the cause: {detail}"
    );
    assert!(
        detail.contains("probe.core.Attachments"),
        "and the path that reaches it: {detail}"
    );

    let core = artifact(&synthesis, "types/core/core.go");
    assert!(
        !core.contains("Attachments"),
        "a refused capability must not be emitted anyway:\n{core}"
    );

    // And the plan still says the capability is generated: the refusal is a fact about the
    // language, recorded at the target stage, never a fact about the model.
    assert!(
        synthesis
            .plan
            .is_generated(CapabilityKind::DomainType, "probe.core.Attachments"),
        "the planner must not have been taught about Go"
    );
}

/// A specification with one component accepting two same-named commands from two contexts.
fn colliding_seams() -> EssIr {
    let context = |domain: &str| {
        format!(
            "domain: twice.{domain}\nevents:\n  - name: twice.{domain}.Placed\n    fields:\n      \
             - name: note\n        type: String\ncommands:\n  - name: twice.{domain}.Place\n    \
             input:\n      - name: note\n        type: String\n    outcomes:\n      - name: \
             done\n        emits:\n          - twice.{domain}.Placed\n"
        )
    };
    fixture(&[
        (
            "system.yaml",
            "format: ess/1\nsystem: twice\nversion: v1\ndomains:\n  - twice.alpha\n  - \
             twice.beta\n",
        ),
        ("alpha.yaml", &context("alpha")),
        ("beta.yaml", &context("beta")),
        (
            "wiring.yaml",
            "components:\n  - component: desk\n    owns:\n      domains:\n        - \
             twice.alpha\n        - twice.beta\n    accepts:\n      commands:\n        - \
             twice.alpha.Place\n        - twice.beta.Place\n    publishes:\n      events:\n        \
             - twice.alpha.Placed\n        - twice.beta.Placed\n",
        ),
    ])
}

#[test]
fn two_seams_of_one_component_that_derive_one_method_name_are_refused_not_renamed() {
    // Go gives a type one method set, and Rust disambiguates a trait method by its trait. A
    // rename would be this emitter choosing a name the specification did not, which is the
    // guessing the whole design refuses; the honest answer is a target-stage refusal.
    let synthesis = synthesize_for(&colliding_seams(), Target::Go);
    let report = synthesis
        .target
        .as_ref()
        .expect("the Go target writes a report");
    let refusal = report
        .refusals
        .iter()
        .find(|refusal| refusal.capability.kind == CapabilityKind::ComponentPort)
        .expect("the port is refused");
    assert_eq!(refusal.capability.source, "desk");
    assert!(
        refusal.detail.contains("one method set"),
        "the refusal names the target's own rule: {}",
        refusal.detail
    );
    assert!(
        refusal.detail.contains("twice.alpha.Place") && refusal.detail.contains("twice.beta.Place"),
        "and both colliding seams: {}",
        refusal.detail
    );
    assert!(
        !synthesis.artifacts.contains_key("components/desk/desk.go"),
        "a refused port must not be emitted anyway"
    );
}

// ---- determinism -----------------------------------------------------------------------------------

#[test]
fn emitting_twice_is_byte_identical() {
    // Invariant 9 in the only form it is worth anything: two independent compilations and
    // syntheses of the same source, compared byte for byte across every artifact.
    let first = synthesize_for(&billing(), Target::Go);
    let second = synthesize_for(&billing(), Target::Go);
    assert_eq!(
        first.artifacts.keys().collect::<Vec<_>>(),
        second.artifacts.keys().collect::<Vec<_>>(),
        "two syntheses produced different file sets"
    );
    for (path, emitted) in &first.artifacts {
        assert_eq!(
            emitted.contents, second.artifacts[path].contents,
            "`{path}` differs between two syntheses of one specification"
        );
    }
    assert_eq!(
        first.target.map(|report| report.to_canonical_json()),
        second.target.map(|report| report.to_canonical_json()),
        "two target reports of one specification differ"
    );
}

#[test]
fn every_artifact_names_its_specification_and_the_verb_that_rewrites_it() {
    for emitted in go().artifacts.values() {
        if is_json(&emitted.path) {
            // JSON has no comments; both JSON artifacts carry provenance as data instead, and the
            // Markdown that always travels beside each names the verb.
            let parsed: serde_json::Value =
                serde_json::from_str(&emitted.contents).expect("the document is valid JSON");
            assert_eq!(
                parsed["provenance"]["system"], "billing",
                "{}",
                emitted.path
            );
            assert_eq!(
                parsed["provenance"]["specification_version"], "v3",
                "{}",
                emitted.path
            );
            continue;
        }
        assert!(
            emitted.contents.contains("generated from billing v3"),
            "`{}` does not say which specification produced it",
            emitted.path
        );
        assert!(
            emitted.contents.contains("protocol ess synthesize"),
            "`{}` does not say what rewrites it",
            emitted.path
        );
    }
}

#[test]
fn no_go_source_uses_a_tab_free_indent_or_a_trailing_space() {
    // `gofmt -l` is the gate's check and it needs a toolchain; these two rules are the ones a
    // hand-written format string breaks, and they cost nothing to hold here as well.
    for emitted in go().artifacts.values() {
        if !is_go(&emitted.path) {
            continue;
        }
        for (number, line) in emitted.contents.lines().enumerate() {
            assert!(
                !line.starts_with(' '),
                "{}:{} is indented with spaces; gofmt indents with tabs",
                emitted.path,
                number + 1
            );
            assert!(
                !line.ends_with(' ') && !line.ends_with('\t'),
                "{}:{} ends in whitespace, which gofmt strips",
                emitted.path,
                number + 1
            );
        }
    }
}
