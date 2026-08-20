//! The billing specification, planned and emitted from the files it actually lives in.
//!
//! Wave 6's acceptance criteria for this slice, executed rather than asserted: the plan lists
//! every construct with a disposition and zero guesses, emitting twice is byte-identical, and the
//! emitted lifecycle surface refuses — at the API level these tests can see, and at compile time
//! for anyone who links the workspace — every transition the specification refuses.
//!
//! # Why the compile-time claim is tested as an API-surface scan
//!
//! The claim "an illegal transition does not compile" wants a compile-fail test, and the crate for
//! that is `trybuild` — a new dev-dependency, plus a copy of the generated code arranged for it to
//! chew on, plus its compiler-version-sensitive `.stderr` fixtures. What the dependency would buy
//! is already implied by two cheaper facts this file checks directly: the only constructor is on
//! the initial state, and the only `impl` blocks that change `S` are exactly the declared
//! transitions. If no API exists that expresses the illegal move, no caller can compile one — the
//! same reasoning `crates/aep-domain/tests/invariants.rs` records for refusing `trybuild`, reused
//! deliberately rather than re-litigated.

use std::path::{Path, PathBuf};

use ess_compiler::ir::EssIr;
use ess_compiler::resolve::compile_locating;
use ess_compiler::source::SourceMap;
use ess_domain::spec::{RawSpecFile, Specification};
use ess_domain::system::Source;
use ess_synth::{
    synthesize, CapabilityKind, ObligationReason, RefusalReason, RefusalStage,
    SynthesisDisposition, SynthesisPlan,
};

/// The example directory.
fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/billing")
        .canonicalize()
        .expect("the billing example exists")
}

/// Every `.yaml` file in the example, relative to it, in a stable order — discovered rather than
/// listed, so a file added to the example cannot be silently ignored here.
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

/// One generated artifact's contents, by path.
fn artifact(synthesis: &ess_synth::Synthesis, path: &str) -> String {
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

// ---- the plan -----------------------------------------------------------------------------------

#[test]
fn the_billing_plan_gives_every_capability_exactly_one_disposition() {
    let plan = SynthesisPlan::of(&billing());
    let mut seen = std::collections::BTreeSet::new();
    for planned in &plan.capabilities {
        assert!(
            seen.insert((planned.capability.kind, planned.capability.source.clone())),
            "`{}` ({:?}) carries two dispositions, and the plan's whole promise is exactly one",
            planned.capability.source,
            planned.capability.kind
        );
    }
}

#[test]
fn the_billing_plan_counts_are_pinned() {
    // Pinned as numbers, deliberately: a construct that silently gains or loses a disposition
    // must be a failing test and an approved diff, not a quietly different document.
    let plan = SynthesisPlan::of(&billing());
    let counts = plan.counts();
    assert_eq!(plan.capabilities.len(), 43, "capabilities in total");
    assert_eq!(counts.generated, 29, "generated capabilities");
    assert_eq!(counts.obligations, 7, "obligations");
    assert_eq!(counts.refused, 7, "refusals");
}

#[test]
fn every_construct_of_the_specification_appears_in_the_plan() {
    let ir = billing();
    let plan = SynthesisPlan::of(&ir);
    let mut missing = Vec::new();
    let mut require = |kind: CapabilityKind, source: String| {
        if plan.disposition_of(kind, &source).is_none() {
            missing.push(format!("{kind:?} {source}"));
        }
    };
    for name in ir.types.keys() {
        require(CapabilityKind::DomainType, name.to_string());
    }
    for name in ir.entities.keys() {
        require(CapabilityKind::EntityLifecycle, name.to_string());
    }
    for name in ir.commands.keys() {
        require(CapabilityKind::CommandContract, name.to_string());
        require(CapabilityKind::CommandBehavior, name.to_string());
    }
    for name in ir.events.keys() {
        require(CapabilityKind::EventType, name.to_string());
    }
    for name in ir.errors.keys() {
        require(CapabilityKind::ErrorType, name.to_string());
    }
    for name in ir.views.keys() {
        require(CapabilityKind::ViewType, name.to_string());
        require(CapabilityKind::ViewQuery, name.to_string());
    }
    for name in ir.actors.keys() {
        require(CapabilityKind::ActorGrants, name.to_string());
    }
    for name in ir.bindings.keys() {
        require(CapabilityKind::Binding, name.to_string());
    }
    for name in ir.components.keys() {
        require(CapabilityKind::ComponentPort, name.to_string());
        require(CapabilityKind::Workload, name.to_string());
    }
    assert_eq!(ir.conversions.len(), 1, "the example declares one crossing");
    require(
        CapabilityKind::Conversion,
        "billing.invoice.Email -> billing.email.EmailAddress".to_owned(),
    );
    assert!(
        missing.is_empty(),
        "constructs with no disposition — the plan is lying by omission: {missing:?}"
    );
}

#[test]
fn send_email_behaviour_is_owed_with_the_specifications_own_cause() {
    // The no-guessing rule at its sharpest: the spec says the `failed` outcome is decided by the
    // provider, so the behaviour cannot be generated — and the reason must carry the author's own
    // words, not a paraphrase the planner invented.
    let plan = SynthesisPlan::of(&billing());
    let disposition = plan
        .disposition_of(CapabilityKind::CommandBehavior, "billing.email.SendEmail")
        .expect("SendEmail has a behaviour capability");
    let SynthesisDisposition::Obligation(obligation) = disposition else {
        panic!("SendEmail's behaviour must be an obligation, not {disposition:?}");
    };
    let ObligationReason::External { cause } = &obligation.reason else {
        panic!(
            "SendEmail's behaviour is externally decided, not {:?}",
            obligation.reason
        );
    };
    assert_eq!(cause, "the provider rejects the recipient address");
    assert!(
        obligation.contract.contains("`failed`")
            && obligation.contract.contains("billing.email.Undeliverable"),
        "the contract names the refusal branch and its error: {}",
        obligation.contract
    );
}

#[test]
fn a_view_query_obligation_carries_filter_and_consistency() {
    let plan = SynthesisPlan::of(&billing());
    let disposition = plan
        .disposition_of(
            CapabilityKind::ViewQuery,
            "billing.invoice.OutstandingInvoices",
        )
        .expect("the view has a query capability");
    let SynthesisDisposition::Obligation(obligation) = disposition else {
        panic!("a view query is owed, not {disposition:?}");
    };
    assert!(
        obligation.contract.contains("read_your_writes")
            && obligation.contract.contains("state == Issued"),
        "the contract must carry the declared consistency and the filter: {}",
        obligation.contract
    );
}

#[test]
fn grants_are_refused_rather_than_owed() {
    // Review H8, taken as this wave's decision on design §28: nothing grant-shaped may be derived
    // from a plan. An *obligation* for an actor would be exactly that derivation.
    let plan = SynthesisPlan::of(&billing());
    for actor in ["billing.invoice.Customer", "billing.invoice.Auditor"] {
        let disposition = plan
            .disposition_of(CapabilityKind::ActorGrants, actor)
            .expect("every actor appears in the plan");
        let SynthesisDisposition::Refused(refusal) = disposition else {
            panic!("`{actor}` must be refused, not {disposition:?}");
        };
        assert_eq!(refusal.reason, RefusalReason::NeedsCallerIdentity);
        assert_eq!(
            refusal.stage,
            RefusalStage::Planning,
            "a grant refusal holds for every target, so it is the planner's"
        );
    }
}

#[test]
fn the_binding_is_refused_at_the_planning_stage_with_its_facts() {
    let plan = SynthesisPlan::of(&billing());
    let disposition = plan
        .disposition_of(CapabilityKind::Binding, "notify-on-invoice-created")
        .expect("the binding appears in the plan");
    let SynthesisDisposition::Refused(refusal) = disposition else {
        panic!("the binding must be refused in this scope, not {disposition:?}");
    };
    assert_eq!(refusal.reason, RefusalReason::NeedsInteractionLayer);
    assert!(
        refusal.detail.contains("billing.invoice.InvoiceCreated")
            && refusal.detail.contains("billing.email.SendEmail"),
        "a refusal names the construct's own facts: {}",
        refusal.detail
    );
}

#[test]
fn a_mechanical_conversion_is_generated_and_any_other_declared_crossing_is_owed() {
    // The billing crossing joins two newtypes over one representation, so it is fully determined.
    let plan = SynthesisPlan::of(&billing());
    assert_eq!(
        plan.disposition_of(
            CapabilityKind::Conversion,
            "billing.invoice.Email -> billing.email.EmailAddress",
        ),
        Some(&SynthesisDisposition::Generated),
        "a newtype-to-newtype crossing over one representation is mechanical"
    );

    // The fixture's crossing joins a Uuid-backed newtype to a String-backed one: permitted by the
    // author, but the computation between the representations is nowhere declared — so generating
    // one would be a guess, and the disposition must be an obligation instead.
    let ir = fixture(&[
        (
            "system.yaml",
            "format: ess/1\nsystem: ledger\nversion: v1\ndomains:\n  - ledger.core\n",
        ),
        (
            "core.yaml",
            "domain: ledger.core\ntypes:\n  - name: ledger.core.AccountId\n    kind: newtype\n    \
             of: Uuid\n  - name: ledger.core.AccountRef\n    kind: newtype\n    of: \
             String\nconversions:\n  - from: ledger.core.AccountId\n    to: \
             ledger.core.AccountRef\n    because: an account may be referred to by its id \
             rendered as text.\n",
        ),
    ]);
    let plan = SynthesisPlan::of(&ir);
    let disposition = plan
        .disposition_of(
            CapabilityKind::Conversion,
            "ledger.core.AccountId -> ledger.core.AccountRef",
        )
        .expect("the fixture's crossing appears in the plan");
    let SynthesisDisposition::Obligation(obligation) = disposition else {
        panic!("a crossing between different representations must be owed, not {disposition:?}");
    };
    assert_eq!(obligation.reason, ObligationReason::UnspecifiedAlgorithm);
    assert!(
        obligation.contract.contains("rendered as text"),
        "the contract carries the author's stated reason for the crossing: {}",
        obligation.contract
    );
}

#[test]
fn the_plan_never_names_the_emission_language() {
    // The operator's constraint, pinned: the plan and its renderings are language-neutral, so a
    // Go emitter can consume the same document. `trust` contains `rust`, so the scan respects
    // word boundaries instead of substrings.
    let synthesis = synthesize(&billing());
    for rendering in [
        synthesis.plan.to_markdown(),
        synthesis.plan.to_canonical_json(),
    ] {
        let lower = rendering.to_lowercase();
        let mut from = 0;
        while let Some(position) = lower[from..].find("rust") {
            let at = from + position;
            let before = lower[..at].chars().next_back();
            let after = lower[at + 4..].chars().next();
            assert!(
                before.is_some_and(char::is_alphanumeric)
                    || after.is_some_and(char::is_alphanumeric),
                "the plan names the emission language, which belongs behind the emitter seam: \
                 ...{}...",
                &rendering[at.saturating_sub(40)..(at + 44).min(rendering.len())]
            );
            from = at + 4;
        }
    }
}

// ---- the emitted workspace ----------------------------------------------------------------------

#[test]
fn emitting_twice_is_byte_identical() {
    // Invariant 9 in the only form it is worth anything: two independent compilations and
    // syntheses of the same source, compared byte for byte across every artifact.
    let first = synthesize(&billing());
    let second = synthesize(&billing());
    assert_eq!(
        first.plan, second.plan,
        "two plans of one specification differ"
    );
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
}

#[test]
fn the_legal_transitions_are_the_whole_transition_api() {
    let synthesis = synthesize(&billing());
    let invoice = artifact(&synthesis, "crates/billing-types/src/invoice.rs");

    // Every declared move exists, on exactly the state it starts from.
    for (block, method) in [
        (
            "impl Invoice<invoice_state::Draft> {",
            "pub fn issue(self) -> Invoice<invoice_state::Issued>",
        ),
        (
            "impl Invoice<invoice_state::Draft> {",
            "pub fn cancel(self) -> Invoice<invoice_state::Cancelled>",
        ),
        (
            "impl Invoice<invoice_state::Issued> {",
            "pub fn settle(self) -> Invoice<invoice_state::Paid>",
        ),
        (
            "impl Invoice<invoice_state::Issued> {",
            "pub fn cancel(self) -> Invoice<invoice_state::Cancelled>",
        ),
    ] {
        assert!(
            invoice.contains(block) && invoice.contains(method),
            "the declared transition `{method}` is missing from the generated surface"
        );
    }

    // The refusals, by construction: the two terminal states expose no transition at all, so
    // `Paid → Cancelled` — the specification's own worked example of an illegal move — is not an
    // error case in the generated code, it is a method that does not exist to call.
    for terminal in ["Paid", "Cancelled"] {
        assert!(
            !invoice.contains(&format!("impl Invoice<invoice_state::{terminal}> {{")),
            "`{terminal}` is terminal, so no impl block may offer a move out of it"
        );
    }

    // `settle` exists exactly once — on `Issued` — so no other state gained it by accident.
    assert_eq!(
        invoice.matches("pub fn settle").count(),
        1,
        "`settle` must be offered by exactly one state, the one its transition starts from"
    );
}

#[test]
fn only_the_initial_state_can_be_constructed() {
    // The other half of "an illegal transition does not compile": if `Invoice<Paid>` could be
    // built directly, refusing the method would refuse nothing. One constructor, on `Draft`; every
    // other typed instance comes from `refine`, whose arms are the declared states.
    let synthesis = synthesize(&billing());
    let invoice = artifact(&synthesis, "crates/billing-types/src/invoice.rs");
    assert_eq!(
        invoice.matches("pub fn new").count(),
        1,
        "exactly one constructor exists"
    );
    assert!(
        invoice.contains("impl Invoice<invoice_state::Draft> {\n    /// A new instance"),
        "and it rests on the lifecycle's initial state"
    );
    assert!(
        invoice.contains("pub struct Invoice<S: invoice_state::Marker> {\n    data:"),
        "the typed entity's fields are private, so a literal cannot bypass the constructor"
    );
}

#[test]
fn a_command_outcome_enum_keeps_the_refusal_beside_the_success() {
    let synthesis = synthesize(&billing());
    let invoice = artifact(&synthesis, "crates/billing-types/src/invoice.rs");
    assert!(
        invoice.contains("pub enum CreateInvoiceOutcome"),
        "one outcome enum per command"
    );
    assert!(
        invoice.contains("invoice_created: InvoiceCreated"),
        "the accepted branch carries the event it publishes"
    );
    assert!(
        invoice.contains("error: InvalidAmount"),
        "the rejected branch carries the declared error, not a stringly one"
    );
}

#[test]
fn newtypes_stay_distinct_and_the_declared_crossing_is_the_only_bridge() {
    let synthesis = synthesize(&billing());
    let invoice = artifact(&synthesis, "crates/billing-types/src/invoice.rs");
    let email = artifact(&synthesis, "crates/billing-types/src/email.rs");
    assert!(
        invoice.contains("pub struct Email(pub String);")
            && email.contains("pub struct EmailAddress(pub String);"),
        "both sides stay wrappers, distinct from String and from each other"
    );
    assert!(
        email.contains("impl From<crate::invoice::Email> for EmailAddress"),
        "the declared crossing is generated where the destination lives"
    );
    assert!(
        email.contains("validates it again on the way out"),
        "and it carries the author's stated reason for permitting it"
    );
}

#[test]
fn every_artifact_names_its_specification_and_the_verb_that_rewrites_it() {
    let synthesis = synthesize(&billing());
    for (path, emitted) in &synthesis.artifacts {
        if path == ess_synth::PLAN_JSON {
            // JSON has no comments; this one carries provenance as data instead, and the verb is
            // named by the `PLAN.md` that always travels beside it.
            let parsed: serde_json::Value =
                serde_json::from_str(&emitted.contents).expect("the plan is valid JSON");
            assert_eq!(parsed["provenance"]["system"], "billing");
            assert_eq!(parsed["provenance"]["specification_version"], "v3");
            continue;
        }
        assert!(
            emitted.contents.contains("generated from billing v3"),
            "`{path}` does not say which specification produced it"
        );
        assert!(
            emitted.contents.contains("protocol ess synthesize"),
            "`{path}` does not name the command that regenerates it — a reader would run the \
             wrong verb"
        );
    }
}

#[test]
fn colliding_domain_modules_are_renamed_by_rule_not_by_luck() {
    // Two domains sharing a last segment would both claim one module file; the rule switches
    // every domain to its full name, deterministically.
    let ir = fixture(&[
        (
            "system.yaml",
            "format: ess/1\nsystem: duo\nversion: v1\ndomains:\n  - duo.alpha.pay\n  - \
             duo.beta.pay\n",
        ),
        (
            "alpha.yaml",
            "domain: duo.alpha.pay\ntypes:\n  - name: duo.alpha.pay.Token\n    kind: newtype\n    \
             of: String\n",
        ),
        (
            "beta.yaml",
            "domain: duo.beta.pay\ntypes:\n  - name: duo.beta.pay.Token\n    kind: newtype\n    \
             of: String\n",
        ),
    ]);
    let synthesis = synthesize(&ir);
    assert!(
        synthesis
            .artifacts
            .contains_key("crates/duo-types/src/alpha_pay.rs")
            && synthesis
                .artifacts
                .contains_key("crates/duo-types/src/beta_pay.rs"),
        "colliding domains land in full-name modules; got {:?}",
        synthesis.artifacts.keys().collect::<Vec<_>>()
    );
}

#[test]
fn a_domain_named_primitives_cannot_shadow_the_representation_module() {
    let ir = fixture(&[
        (
            "system.yaml",
            "format: ess/1\nsystem: demo\nversion: v1\ndomains:\n  - demo.primitives\n",
        ),
        (
            "primitives.yaml",
            "domain: demo.primitives\ntypes:\n  - name: demo.primitives.Token\n    kind: \
             newtype\n    of: String\n",
        ),
    ]);
    let synthesis = synthesize(&ir);
    assert!(
        synthesis
            .artifacts
            .contains_key("crates/demo-types/src/primitives_domain.rs"),
        "`primitives` is reserved for the representation module; got {:?}",
        synthesis.artifacts.keys().collect::<Vec<_>>()
    );
    let lib = artifact(&synthesis, "crates/demo-types/src/lib.rs");
    assert!(
        lib.contains("pub mod primitives;") && lib.contains("pub mod primitives_domain;"),
        "both modules exist, distinctly: {lib}"
    );
}

// ---- the crate's own determinism hygiene --------------------------------------------------------

#[test]
fn no_source_file_in_this_crate_reads_a_clock_or_an_unordered_map() {
    // The same scan `ess-compiler` runs over itself, for the same reason: the determinism the
    // byte-comparison test observes has to be load-bearing in the sources, and an unordered map
    // introduced next month would be invisible to a reviewer who trusts the passing test on the
    // old code paths.
    let sources = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let banned = [
        "HashMap",
        "HashSet",
        "SystemTime",
        "Instant::now",
        "rand::",
        "thread_rng",
    ];
    let mut pending = vec![sources];
    let mut scanned = 0;
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("the sources are readable") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path.extension().is_none_or(|extension| extension != "rs") {
                continue;
            }
            scanned += 1;
            let text = std::fs::read_to_string(&path).expect("a source file is readable");
            for token in banned {
                assert!(
                    !text.contains(token),
                    "{} mentions `{token}`, which invariant 9 bans from a generator",
                    path.display()
                );
            }
        }
    }
    assert!(
        scanned >= 6,
        "the scan found only {scanned} source files, so it is probably not looking at the crate"
    );
}
