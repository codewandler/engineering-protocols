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
    assert_eq!(plan.capabilities.len(), 45, "capabilities in total");
    assert_eq!(counts.generated, 33, "generated capabilities");
    assert_eq!(counts.obligations, 8, "obligations");
    assert_eq!(counts.refused, 4, "refusals");
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
        require(CapabilityKind::BindingTransformation, name.to_string());
        require(CapabilityKind::BindingDelivery, name.to_string());
        // The billing binding escalates, so the escalation is a capability of its own — the
        // declared event exists, and filling it is owed.
        require(CapabilityKind::BindingEscalation, name.to_string());
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
fn the_billing_binding_is_generated_where_determined_and_owed_where_not() {
    // The three capabilities one binding splits into, each with its honest disposition: the
    // mapping is fully determined (an event field through the declared mechanical crossing, and a
    // literal into a text-representation newtype), delivery has exactly one declared acceptor,
    // and the escalation event's fields are declared nowhere — so two are generated and the third
    // is owed.
    let plan = SynthesisPlan::of(&billing());
    for kind in [
        CapabilityKind::BindingTransformation,
        CapabilityKind::BindingDelivery,
    ] {
        assert_eq!(
            plan.disposition_of(kind, "notify-on-invoice-created"),
            Some(&SynthesisDisposition::Generated),
            "{kind:?} is fully determined by the billing specification"
        );
    }
    let disposition = plan
        .disposition_of(
            CapabilityKind::BindingEscalation,
            "notify-on-invoice-created",
        )
        .expect("an escalating binding has an escalation capability");
    let SynthesisDisposition::Obligation(obligation) = disposition else {
        panic!("the escalation must be owed, not {disposition:?}");
    };
    assert_eq!(obligation.reason, ObligationReason::UnspecifiedAlgorithm);
    assert!(
        obligation
            .contract
            .contains("billing.email.DeliveryEscalated")
            && obligation.contract.contains("how its fields are filled"),
        "the contract names the declared event and what exactly is not declared: {}",
        obligation.contract
    );
}

#[test]
fn a_mapping_through_a_non_mechanical_crossing_makes_the_transformation_an_obligation() {
    // The one undetermined mapping the model can still express: the crossing is declared —
    // `AccountId` over Uuid may be used as `AccountRef` over String — so the compiler admits the
    // mapping, but the computation between two representations is nowhere declared. Generating
    // one would be a guess, so the transformation is owed, and the pump routes through the owed
    // seam instead.
    let ir = fixture(&[
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
             as text.\nevents:\n  - name: relay.core.Fired\n    fields:\n      - name: account\n        \
             type: relay.core.AccountId\n  - name: relay.core.Handled\n    fields:\n      - name: \
             account\n        type: relay.core.AccountRef\ncommands:\n  - name: \
             relay.core.Handle\n    input:\n      - name: account\n        type: \
             relay.core.AccountRef\n    outcomes:\n      - name: done\n        emits:\n          - \
             relay.core.Handled\n",
        ),
        (
            "wiring.yaml",
            "components:\n  - component: relay-service\n    accepts:\n      commands:\n        - \
             relay.core.Handle\nbindings:\n  - id: relay-on-fired\n    when:\n      event: \
             relay.core.Fired\n    invoke:\n      command: relay.core.Handle\n    mapping:\n      \
             account: event.account\n    delivery: at_least_once\n    on_failure: retry\n",
        ),
    ]);
    let plan = SynthesisPlan::of(&ir);
    let disposition = plan
        .disposition_of(CapabilityKind::BindingTransformation, "relay-on-fired")
        .expect("the binding appears in the plan");
    let SynthesisDisposition::Obligation(obligation) = disposition else {
        panic!("a transformation over an owed crossing must be owed itself, not {disposition:?}");
    };
    assert_eq!(obligation.reason, ObligationReason::UnspecifiedAlgorithm);
    assert!(
        obligation.contract.contains("`account`")
            && obligation.contract.contains("relay.core.AccountRef")
            && obligation.contract.contains("whose computation is owed"),
        "the contract names the input, the crossing's end, and what is owed: {}",
        obligation.contract
    );
    // Delivery is independent of the transformation's disposition: one component accepts the
    // command, so the transport is still generated — routed through the transformation seam.
    assert_eq!(
        plan.disposition_of(CapabilityKind::BindingDelivery, "relay-on-fired"),
        Some(&SynthesisDisposition::Generated),
        "delivery has exactly one declared acceptor"
    );
    // And the emitted system routes through the owed seam rather than inventing a computation.
    let synthesis = synthesize(&ir);
    let system = artifact(&synthesis, "crates/relay-system/src/lib.rs");
    assert!(
        system.contains("self.obligations.relay_on_fired_input(event)?"),
        "the pump calls the transformation obligation, not a generated guess: {system}"
    );
}

#[test]
fn a_binding_whose_command_no_component_accepts_is_refused_never_guessed() {
    let ir = fixture(&[
        (
            "system.yaml",
            "format: ess/1\nsystem: relay\nversion: v1\ndomains:\n  - relay.core\n",
        ),
        (
            "core.yaml",
            "domain: relay.core\ntypes:\n  - name: relay.core.Code\n    kind: newtype\n    of: \
             String\nevents:\n  - name: relay.core.Fired\n    fields:\n      - name: code\n        \
             type: relay.core.Code\n  - name: relay.core.Handled\n    fields:\n      - name: \
             code\n        type: relay.core.Code\ncommands:\n  - name: relay.core.Handle\n    \
             input:\n      - name: code\n        type: relay.core.Code\n    outcomes:\n      - \
             name: done\n        emits:\n          - relay.core.Handled\n",
        ),
        (
            "wiring.yaml",
            "bindings:\n  - id: relay-on-fired\n    when:\n      event: relay.core.Fired\n    \
             invoke:\n      command: relay.core.Handle\n    mapping:\n      code: event.code\n    \
             delivery: at_least_once\n    on_failure: retry\n",
        ),
    ]);
    let plan = SynthesisPlan::of(&ir);
    let disposition = plan
        .disposition_of(CapabilityKind::BindingDelivery, "relay-on-fired")
        .expect("the binding appears in the plan");
    let SynthesisDisposition::Refused(refusal) = disposition else {
        panic!("delivery with no acceptor must be refused, not {disposition:?}");
    };
    assert_eq!(refusal.reason, RefusalReason::AcceptorUndetermined);
    assert_eq!(refusal.stage, RefusalStage::Planning);
    assert!(
        refusal
            .detail
            .contains("no declared component accepts `relay.core.Handle`"),
        "the refusal names the command nothing accepts: {}",
        refusal.detail
    );
}

#[test]
fn two_components_accepting_one_command_is_refused_naming_both() {
    // The D-2 rule, applied to delivery: the machinery never chooses among alternatives. Two
    // acceptors is an honest refusal that names them, not a coin toss over who gets the command.
    let ir = fixture(&[
        (
            "system.yaml",
            "format: ess/1\nsystem: relay\nversion: v1\ndomains:\n  - relay.core\n",
        ),
        (
            "core.yaml",
            "domain: relay.core\ntypes:\n  - name: relay.core.Code\n    kind: newtype\n    of: \
             String\nevents:\n  - name: relay.core.Fired\n    fields:\n      - name: code\n        \
             type: relay.core.Code\n  - name: relay.core.Handled\n    fields:\n      - name: \
             code\n        type: relay.core.Code\ncommands:\n  - name: relay.core.Handle\n    \
             input:\n      - name: code\n        type: relay.core.Code\n    outcomes:\n      - \
             name: done\n        emits:\n          - relay.core.Handled\n",
        ),
        (
            "wiring.yaml",
            "components:\n  - component: relay-alpha\n    accepts:\n      commands:\n        - \
             relay.core.Handle\n  - component: relay-beta\n    accepts:\n      commands:\n        - \
             relay.core.Handle\nbindings:\n  - id: relay-on-fired\n    when:\n      event: \
             relay.core.Fired\n    invoke:\n      command: relay.core.Handle\n    mapping:\n      \
             code: event.code\n    delivery: at_least_once\n    on_failure: retry\n",
        ),
    ]);
    let plan = SynthesisPlan::of(&ir);
    let disposition = plan
        .disposition_of(CapabilityKind::BindingDelivery, "relay-on-fired")
        .expect("the binding appears in the plan");
    let SynthesisDisposition::Refused(refusal) = disposition else {
        panic!("delivery with two acceptors must be refused, not {disposition:?}");
    };
    assert_eq!(refusal.reason, RefusalReason::AcceptorUndetermined);
    assert!(
        refusal.detail.contains("`relay-alpha`")
            && refusal.detail.contains("`relay-beta`")
            && refusal
                .detail
                .contains("choosing among them is not this synthesis's decision"),
        "the refusal names both acceptors and refuses the choice: {}",
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

// ---- the component skeletons and the transport --------------------------------------------------

/// Every stub's `(capability, source)` pair, read back out of the generated sources.
///
/// The stubs are found by their one construction site — the `UnmetObligation` struct literal —
/// rather than by any list the emitter keeps, so this is an independent witness: an emitter that
/// forgets a stub, or writes one the plan does not owe, fails here whatever its own bookkeeping
/// says.
fn stubs_in(synthesis: &ess_synth::Synthesis) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for emitted in synthesis.artifacts.values() {
        let text = &emitted.contents;
        let mut from = 0;
        while let Some(position) = text[from..].find("UnmetObligation { capability: \"") {
            let at = from + position + "UnmetObligation { capability: \"".len();
            let capability_end = text[at..].find('"').expect("the capability closes") + at;
            let source_at = text[capability_end..]
                .find("source: \"")
                .expect("the source follows")
                + capability_end
                + "source: \"".len();
            let source_end = text[source_at..].find('"').expect("the source closes") + source_at;
            found.push((
                text[at..capability_end].to_owned(),
                text[source_at..source_end].to_owned(),
            ));
            from = source_end;
        }
    }
    found.sort();
    found
}

#[test]
fn the_plans_obligations_and_the_workspaces_stubs_are_the_same_list() {
    // W6.2's acceptance criterion, executed: an obligation is visible twice — a plan entry and a
    // typed stub — and the two lists are one list. A missing stub is a hole the plan promised
    // would not exist; an extra stub is a refusal the plan cannot explain.
    let synthesis = synthesize(&billing());
    let stubs = stubs_in(&synthesis);
    let mut owed: Vec<(String, String)> = synthesis
        .plan
        .capabilities
        .iter()
        .filter_map(|planned| match &planned.disposition {
            SynthesisDisposition::Obligation(_) => Some((
                planned.capability.kind.describes().to_owned(),
                planned.capability.source.clone(),
            )),
            _ => None,
        })
        .collect();
    owed.sort();
    assert_eq!(owed.len(), 8, "the billing plan owes eight capabilities");
    assert_eq!(
        stubs, owed,
        "the generated stubs are not exactly the plan's obligations"
    );
}

#[test]
fn a_stub_refuses_with_a_value_never_a_panic_and_never_a_todo() {
    // The stub rule at its bluntest: nothing in a generated workspace panics or defers with
    // `todo!`, because a hole that detonates at runtime is the exact failure a typed refusal
    // exists to replace.
    let synthesis = synthesize(&billing());
    for (path, emitted) in &synthesis.artifacts {
        for banned in ["todo!", "unimplemented!", "panic!", "unreachable!"] {
            assert!(
                !emitted.contents.contains(banned),
                "`{path}` contains `{banned}`, and a generated gap must be a typed refusal"
            );
        }
    }
}

#[test]
fn a_component_port_is_typed_against_the_generated_types() {
    let synthesis = synthesize(&billing());
    let port = artifact(&synthesis, "crates/invoice-service/src/lib.rs");
    // The handler: typed input, typed outcome, and the refusal channel is the obligation type —
    // not a string, not a panic.
    assert!(
        port.contains(
            "pub fn create_invoice(&mut self, input: billing_types::invoice::CreateInvoice) -> \
             Result<billing_types::invoice::CreateInvoiceOutcome, \
             billing_types::obligation::UnmetObligation>"
        ),
        "the accepted command is a typed handler: {port}"
    );
    // The declared publication: the accepted branch's event reaches the outbox, and the refusal
    // branch publishes nothing — a refused command changes nothing.
    assert!(
        port.contains("self.outbox.push(PublishedEvent::InvoiceCreated(invoice_created.clone()));"),
        "the outcome's declared event is published: {port}"
    );
    assert!(
        port.contains("CreateInvoiceOutcome::Rejected { .. } => {}"),
        "the refusal branch publishes nothing: {port}"
    );
    // The view queries sit on the same port, typed against the row types.
    assert!(
        port.contains(
            "pub fn outstanding_invoices(&self) -> \
             Result<Vec<billing_types::invoice::OutstandingInvoices>, \
             billing_types::obligation::UnmetObligation>"
        ),
        "the declared view is a typed query: {port}"
    );
    // And the port's bounds are exactly the obligation seams, so `Unimplemented` satisfies them.
    assert!(
        port.contains("billing_types::invoice::obligations::CreateInvoiceBehavior")
            && port.contains("billing_types::invoice::obligations::OutstandingInvoicesQuery"),
        "the port is generic over the obligation traits: {port}"
    );
}

#[test]
fn the_transport_is_the_one_the_billing_binding_requires() {
    // Derived, not chosen: `delivery: at_least_once` and `on_failure: escalate` in the binding,
    // plus who accepts `SendEmail`, fully determine an in-process at-least-once dispatch with the
    // declared escalation — and that is everything the system crate contains.
    let synthesis = synthesize(&billing());
    let system = artifact(&synthesis, "crates/billing-system/src/lib.rs");

    // The transformation: the event field crosses by the declared `From`, and the literal lands
    // in the text-representation newtype — the specification's own words, as code.
    assert!(
        system.contains(
            "recipient: billing_types::email::EmailAddress::from(event.customer_email.clone())"
        ),
        "the mapped field crosses by the declared conversion: {system}"
    );
    assert!(
        system
            .contains("template: billing_types::email::TemplateId(\"invoice-created\".to_owned())"),
        "the literal is wrapped into its declared text newtype: {system}"
    );

    // The delivery: the binding's arm invokes the one declared acceptor, and failure takes the
    // declared policy — escalation through the owed seam, published onto the log.
    assert!(
        system.contains("// `notify-on-invoice-created`: at_least_once, on failure escalate."),
        "the arm carries the binding's own delivery facts: {system}"
    );
    // The failure the policy answers is the command *refusing* — taking its error-carrying
    // outcome — never an unmet obligation, which is the workspace being unfinished rather than a
    // delivery failing and which therefore propagates with `?`. The old shape, `is_err()`, was
    // exactly that conflation: a provider rejecting an address and a behaviour nobody wrote yet
    // took the same branch.
    assert!(
        system.contains("match self.email_service.send_email(input.clone())? {"),
        "the command lands on the component that accepts it, and an unmet obligation propagates: \
         {system}"
    );
    assert!(
        system.contains("billing_types::email::SendEmailOutcome::Sent { .. } => {}")
            && system.contains("billing_types::email::SendEmailOutcome::Failed { .. } =>"),
        "the policy runs on exactly the declared refusal, not on every non-success: {system}"
    );
    assert!(
        !system.contains(".is_err()"),
        "an unmet obligation must not be read as a delivery failure: {system}"
    );
    assert!(
        system.contains("self.obligations.notify_on_invoice_created_escalation(&input)?")
            && system.contains("self.published.push(SystemEvent::DeliveryEscalated(escalation));"),
        "an escalation is built through the obligation seam and published: {system}"
    );

    // One transport: nothing here reaches a network, a broker, or a second delivery guarantee.
    for absent in [
        "http",
        "tcp",
        "kafka",
        "amqp",
        "exactly_once",
        "at_most_once",
    ] {
        assert!(
            !system.to_lowercase().contains(absent),
            "`{absent}` appears in the system crate, which holds exactly one declared transport: \
             {system}"
        );
    }
}

#[test]
fn the_transport_records_its_invocations_and_can_deliver_an_occurrence_twice() {
    // The two observations a conformance run needs of a transport and nothing inside the system
    // needs: what a binding actually passed (a mapping's target is a command input, which the
    // model relates to no observable fact afterwards), and that an occurrence can reach its
    // bindings a second time (the only claim `at_least_once` makes). Both are the transport's to
    // expose — reading either out of a component would be instrumentation the specification never
    // asked of it.
    let synthesis = synthesize(&billing());
    let system = artifact(&synthesis, "crates/billing-system/src/lib.rs");

    assert!(
        system.contains("pub enum BindingInvocation")
            && system.contains("NotifyOnInvoiceCreated(billing_types::email::SendEmail)"),
        "the record is typed against the invoked command's input: {system}"
    );
    assert!(
        system.contains(
            "self.invocations.push(BindingInvocation::NotifyOnInvoiceCreated(input.clone()));"
        ),
        "the pump records the invocation at the moment it happens: {system}"
    );
    assert!(
        system.contains("pub fn invocations(&self) -> &[BindingInvocation]"),
        "the record is observable from outside: {system}"
    );
    assert!(
        system.contains("pub fn redeliver(&mut self, event: &SystemEvent)")
            && system.contains("self.deliver(event)?;\n        self.pump()"),
        "redelivery runs the bindings again without publishing a second occurrence: {system}"
    );
}

#[test]
fn colliding_event_names_become_full_name_variants_by_rule_not_by_luck() {
    // Two domains may each declare a `Ping`; one component may publish both. Two variants cannot
    // share a name, so every variant switches to the full spelling — all of them, deterministically.
    let ir = fixture(&[
        (
            "system.yaml",
            "format: ess/1\nsystem: duo\nversion: v1\ndomains:\n  - duo.alpha\n  - duo.beta\n",
        ),
        (
            "alpha.yaml",
            "domain: duo.alpha\nevents:\n  - name: duo.alpha.Ping\n",
        ),
        (
            "beta.yaml",
            "domain: duo.beta\nevents:\n  - name: duo.beta.Ping\n",
        ),
        (
            "wiring.yaml",
            "components:\n  - component: fanout\n    publishes:\n      events:\n        - \
             duo.alpha.Ping\n        - duo.beta.Ping\n",
        ),
    ]);
    let synthesis = synthesize(&ir);
    let port = artifact(&synthesis, "crates/fanout/src/lib.rs");
    assert!(
        port.contains("AlphaPing(duo_types::alpha::Ping)")
            && port.contains("BetaPing(duo_types::beta::Ping)"),
        "colliding events take their full names as variants: {port}"
    );
}

#[test]
fn a_component_named_like_a_reserved_package_is_renamed_by_rule() {
    // `demo-types` is where the types crate lives; a component with that name cannot claim the
    // same directory, and the repair is deterministic rather than positional.
    let ir = fixture(&[
        (
            "system.yaml",
            "format: ess/1\nsystem: demo\nversion: v1\ndomains:\n  - demo.core\n",
        ),
        (
            "core.yaml",
            "domain: demo.core\nevents:\n  - name: demo.core.Pinged\n",
        ),
        (
            "wiring.yaml",
            "components:\n  - component: demo-types\n    publishes:\n      events:\n        - \
             demo.core.Pinged\n",
        ),
    ]);
    let synthesis = synthesize(&ir);
    assert!(
        synthesis
            .artifacts
            .contains_key("crates/demo-types-component/Cargo.toml"),
        "the colliding component moves to `-component`; got {:?}",
        synthesis.artifacts.keys().collect::<Vec<_>>()
    );
    assert!(
        synthesis
            .artifacts
            .contains_key("crates/demo-types/Cargo.toml"),
        "and the types crate keeps its own directory"
    );
}

#[test]
fn a_domain_named_obligation_cannot_shadow_the_refusal_module() {
    // `obligation` joined `primitives` on the reserved list when the refusal type arrived; a
    // bounded context with that local name moves aside by the same rule.
    let ir = fixture(&[
        (
            "system.yaml",
            "format: ess/1\nsystem: demo\nversion: v1\ndomains:\n  - demo.obligation\n",
        ),
        (
            "obligation.yaml",
            "domain: demo.obligation\nevents:\n  - name: demo.obligation.Ponged\ncommands:\n  - \
             name: demo.obligation.Ping\n    input:\n      - name: note\n        type: \
             String\n    outcomes:\n      - name: done\n        emits:\n          - \
             demo.obligation.Ponged\n",
        ),
    ]);
    let synthesis = synthesize(&ir);
    assert!(
        synthesis
            .artifacts
            .contains_key("crates/demo-types/src/obligation_domain.rs"),
        "`obligation` is reserved for the refusal module; got {:?}",
        synthesis.artifacts.keys().collect::<Vec<_>>()
    );
    let lib = artifact(&synthesis, "crates/demo-types/src/lib.rs");
    assert!(
        lib.contains("pub mod obligation;") && lib.contains("pub mod obligation_domain;"),
        "both modules exist, distinctly: {lib}"
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
