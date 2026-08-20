//! What `examples/oracle-fixture/` exists to make possible, asserted against the compiled IR.
//!
//! Wave 4 turns a specification into an oracle, and design §50 accepts that milestone only if seven
//! deliberate faults (§25) each fail the one check written to catch them. A fault that cannot be
//! staged is the same defect as a test that cannot fail — which is what a mutation review spent a
//! day finding in this repository — so the inputs those checks run on have to be checked too.
//!
//! `examples/billing/` is the normative example and stays readable as the specification of a real
//! system rather than a corner-case museum, so the corners live in a second fixture. Every row of
//! that fixture's `README.md` is a test below. The point is that a corner cannot be quietly deleted
//! from those YAML files: deleting one fails a test that names it.
//!
//! # Why the IR and not the documents
//!
//! Every property here is a property of what the *compiler* produced. "A row reaches the
//! read-your-writes view" is not a claim about the text `state == Placed`; it is a claim that the
//! view's resolved filter evaluates to [`Truth::True`] against the state a declared `creates:`
//! outcome leaves the entity in. So the filter is evaluated, against a fact source built from the
//! lifecycle, rather than string-matched.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use aep_domain::facts::{FactPath, FactStore, FactValue};
use aep_domain::predicate::Truth;
use ess_compiler::ir::{
    Driver, EntityHandle, EssIr, ResolvedEffect, ResolvedMappingValue, ResolvedView,
};
use ess_compiler::resolve::compile_locating;
use ess_compiler::source::SourceMap;
use ess_domain::binding::Failure;
use ess_domain::command::TestStrategy;
use ess_domain::entity::StateName;
use ess_domain::spec::{RawSpecFile, Specification};
use ess_domain::system::Source;
use ess_domain::view::Consistency;

/// The fixture this file is about.
const FIXTURE: &str = "oracle-fixture";

/// The normative example, compiled beside it so "billing does not have this" stays a fact rather
/// than a comment that was true once.
const NORMATIVE: &str = "billing";

// ---- compiling an example --------------------------------------------------------------------

/// An example directory, by the name it has under `examples/`.
fn example(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name)
        .canonicalize()
        .unwrap_or_else(|error| panic!("examples/{name} exists: {error}"))
}

/// Every `.yaml` file in an example, relative to it, in a stable order.
///
/// Discovered rather than listed, for the reason `tests/billing.rs` gives: a file added to the
/// example would otherwise be compiled by the CLI and ignored by the test meant to keep it honest.
fn files(name: &str) -> Vec<String> {
    let base = example(name);
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
    assert!(!found.is_empty(), "examples/{name} holds no YAML files");
    found.sort();
    found
}

/// An example, compiled from the files it actually lives in.
fn compiled(name: &str) -> EssIr {
    let labels = files(name);
    let mut sources = SourceMap::new();
    let mut parsed = Vec::new();
    for label in &labels {
        let text = std::fs::read_to_string(example(name).join(label))
            .unwrap_or_else(|error| panic!("{name}/{label} is readable: {error}"));
        let raw = RawSpecFile::parse(&text)
            .unwrap_or_else(|error| panic!("{name}/{label} is well formed: {error}"));
        sources.insert(label.clone(), text);
        parsed.push((Source::new(label.clone()), raw));
    }
    let specification = Specification::assemble(parsed)
        .unwrap_or_else(|errors| panic!("examples/{name} validates:\n{errors}"));
    compile_locating(&specification, &sources, &labels)
        .unwrap_or_else(|diagnostics| panic!("examples/{name} resolves:\n{diagnostics}"))
}

// ---- reading the lifecycle the way a scenario would --------------------------------------------

/// Every state a scenario can actually put an entity into, using only declared commands.
///
/// The closure of "a `creates:` outcome exists, so the initial state is reachable" under "a
/// `moves:` outcome exists whose transition starts somewhere already reached". An entity nothing
/// creates has no reachable states at all, which is the honest answer: before gate G14 landed the
/// command-to-transition link, this returned the empty set for every entity in the repository.
fn reachable_states(ir: &EssIr, entity: &EntityHandle) -> BTreeSet<StateName> {
    let lifecycle = &ir.entity(entity).lifecycle;
    let drivers = ir.drivers();
    let none = Vec::new();
    let driving = drivers.get(entity).unwrap_or(&none);

    let mut states = BTreeSet::new();
    if driving
        .iter()
        .any(|driver| matches!(driver.effect, ResolvedEffect::Creates))
    {
        states.insert(lifecycle.initial.clone());
    }
    loop {
        let before = states.len();
        for driver in driving {
            if let Some(transition) = driver.effect.transition() {
                let enabled = transition.from.iter().any(|from| states.contains(from));
                if enabled {
                    states.insert(transition.to.clone());
                }
            }
        }
        if states.len() == before {
            return states;
        }
    }
}

/// A fact source describing one entity instance resting in `state`.
///
/// Only `state` is bound, because that is what every filter in either example reads. A filter that
/// reads anything else fails [`satisfied_at`] loudly rather than evaluating to `Unknown` and being
/// read as "no".
fn resting_in(state: &StateName) -> FactStore {
    let mut facts = FactStore::new();
    facts.set(
        FactPath::new("state").expect("`state` is a fact path"),
        FactValue::text(state.to_string()),
    );
    facts
}

/// Whether a view's filter selects an entity resting in `state`.
///
/// A view with no filter selects everything, so it is satisfied wherever the entity is.
fn satisfied_at(view: &ResolvedView, state: &StateName) -> bool {
    let Some(filter) = &view.filter else {
        return true;
    };
    for path in filter.fact_paths() {
        assert_eq!(
            path.to_string(),
            "state",
            "`{}` filters on `{path}`, which this test cannot supply; bind it in `resting_in` \
             before trusting the answer",
            view.name
        );
    }
    match filter.evaluate(&resting_in(state)) {
        Truth::True => true,
        Truth::False => false,
        Truth::Unknown => panic!(
            "`{}`'s filter is Unknown against a bound `state`, so this test would be reading \
             unobserved as false — which invariant 5 forbids",
            view.name
        ),
    }
}

/// Every state some declared view of `entity` selects on, by view name.
fn selecting_states(ir: &EssIr, entity: &EntityHandle) -> BTreeMap<String, BTreeSet<StateName>> {
    let reachable = reachable_states(ir, entity);
    ir.projections()
        .get(entity)
        .map(|views| {
            views
                .iter()
                .map(|view| {
                    let hits = reachable
                        .iter()
                        .filter(|state| satisfied_at(view, state))
                        .cloned()
                        .collect();
                    (view.name.to_string(), hits)
                })
                .collect()
        })
        .unwrap_or_default()
}

// ---- the properties the fixture exists to provide ----------------------------------------------

#[test]
fn the_fixture_compiles_from_the_files_it_lives_in() {
    let ir = compiled(FIXTURE);

    assert_eq!(
        ir.system.to_string(),
        "oracle",
        "the fixture is the `oracle` system"
    );
    assert_eq!(
        ir.domains.len(),
        2,
        "two bounded contexts, so a binding crosses one: {:?}",
        ir.domains.keys().collect::<Vec<_>>()
    );
    assert!(
        !ir.bindings.is_empty() && !ir.components.is_empty(),
        "a fixture for the oracle without components or bindings specifies nothing executable"
    );
}

#[test]
fn every_on_failure_policy_the_model_has_is_reachable_in_this_fixture() {
    let ir = compiled(FIXTURE);

    let used: Vec<Failure> = ir
        .bindings
        .values()
        .map(|binding| binding.failure)
        .collect();
    let missing: Vec<&str> = Failure::WORDS
        .iter()
        .filter(|word| {
            let policy = Failure::parse(word)
                .unwrap_or_else(|| panic!("`{word}` is in `Failure::WORDS` but parses as nothing"));
            !used.contains(&policy)
        })
        .copied()
        .collect();

    assert!(
        missing.is_empty(),
        "no binding uses {missing:?}; a failure policy no binding declares is a word the oracle \
         has no scenario for, and `drop` in particular is the only input that makes synthesis \
         refuse a check (design §18) rather than emit one"
    );
}

#[test]
fn dropping_one_binding_leaves_others_with_scenarios_of_their_own() {
    let ir = compiled(FIXTURE);
    let reactions = ir.reactions();

    assert!(
        reactions.len() >= 2,
        "every binding here reacts to one event ({:?}), so a dropped-binding fault fails every \
         binding scenario there is; design §26 wants the unrelated ones still passing",
        reactions
            .keys()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    );
    for binding in ir.bindings.values() {
        let elsewhere = ir
            .bindings
            .values()
            .filter(|other| other.event != binding.event)
            .count();
        assert!(
            elsewhere > 0,
            "dropping `{}` leaves no binding on a different event to stay green",
            binding.name
        );
    }
}

#[test]
fn the_command_every_binding_invokes_can_be_forced_to_fail() {
    let ir = compiled(FIXTURE);

    for binding in ir.bindings.values() {
        let command = ir.command(&binding.command);
        let forceable = command
            .outcomes
            .iter()
            .any(|outcome| outcome.test_strategy == TestStrategy::InjectFault);
        assert!(
            forceable,
            "`{}` invokes `{}`, which declares no externally decided outcome — so nothing can \
             force the failure that `on_failure: {}` is the answer to, and the policy is a word no \
             scenario reaches",
            binding.name,
            command.name,
            binding.failure.as_str()
        );
    }
}

#[test]
fn a_row_reaches_the_read_your_writes_view_after_a_single_command() {
    let ir = compiled(FIXTURE);

    let mut checked = 0_usize;
    for view in ir.views.values() {
        if view.consistency != Consistency::ReadYourWrites {
            continue;
        }
        checked += 1;

        // The state a `creates:` outcome leaves a new instance in: reach the state the rule is
        // load-bearing in before asserting anything about it.
        let entity = ir.entity(&view.source);
        let created = ir
            .drivers()
            .get(&view.source)
            .into_iter()
            .flatten()
            .any(|driver| matches!(driver.effect, ResolvedEffect::Creates));
        assert!(
            created,
            "nothing creates `{}`, so no scenario can put a row anywhere near `{}`",
            entity.name, view.name
        );

        assert!(
            satisfied_at(view, &entity.lifecycle.initial),
            "`{}` is read-your-writes and selects nothing at `{}`, the state one command can \
             reach; a stale read of an empty view looks exactly like a fresh one, so F-VIEW-RACE \
             would have nothing to race against",
            view.name,
            entity.lifecycle.initial
        );
    }
    assert!(
        checked > 0,
        "the fixture declares no read-your-writes view, which is the one thing gate G15 names"
    );
}

#[test]
fn the_eventual_view_converges_on_a_state_the_creating_command_does_not_reach() {
    let ir = compiled(FIXTURE);

    let mut checked = 0_usize;
    for view in ir.views.values() {
        if view.consistency != Consistency::Eventual {
            continue;
        }
        checked += 1;

        let entity = ir.entity(&view.source);
        let initial = &entity.lifecycle.initial;
        assert!(
            !satisfied_at(view, initial),
            "`{view_name}` is satisfied at `{initial}`, the state creation leaves the entity in, \
             so a bounded eventual assertion over it is already true before anything converges",
            view_name = view.name
        );

        let later: Vec<String> = reachable_states(&ir, &view.source)
            .into_iter()
            .filter(|state| satisfied_at(view, state))
            .map(|state| state.to_string())
            .collect();
        assert!(
            !later.is_empty(),
            "`{}` selects on no state any declared command reaches, so waiting for it to converge \
             waits forever",
            view.name
        );
    }
    assert!(
        checked > 0,
        "the fixture declares no eventual view, so only one of the two consistency modes has an \
         input here"
    );
}

#[test]
fn an_illegal_transition_can_be_attempted_from_a_state_a_scenario_can_reach() {
    let ir = compiled(FIXTURE);

    let drivers = ir.drivers();
    let mut attempts = Vec::new();
    for (entity, driving) in &drivers {
        for state in reachable_states(&ir, entity) {
            for driver in driving {
                if let Some(transition) = driver.effect.transition() {
                    if !transition.from.contains(&state) {
                        attempts.push(format!(
                            "{} in {state}, then {}",
                            ir.entity(entity).name,
                            driver.command.name
                        ));
                    }
                }
            }
        }
    }

    assert!(
        !attempts.is_empty(),
        "no reachable state has a declared move that may not start there, so design §19's \
         negative lifecycle check has nothing to attempt"
    );
}

#[test]
fn an_outcome_updates_an_entity_without_moving_it_and_that_entity_declares_an_invariant() {
    let ir = compiled(FIXTURE);

    let drivers = ir.drivers();
    let updating: Vec<Driver<'_>> = drivers
        .values()
        .flatten()
        .filter(|driver| matches!(driver.effect, ResolvedEffect::Updates))
        .copied()
        .collect();
    assert!(
        !updating.is_empty(),
        "nothing here uses `updates:`, so design §20's \"evaluate invariants after a \
         state-changing command\" has no instance without a transition to hang the evaluation on"
    );

    for (entity, driving) in &drivers {
        let updates = driving
            .iter()
            .any(|driver| matches!(driver.effect, ResolvedEffect::Updates));
        if updates {
            assert!(
                !ir.entity(entity).invariants.is_empty(),
                "`{}` is updated without moving and declares no invariant, so the update changes \
                 nothing an oracle could check",
                ir.entity(entity).name
            );
        }
    }
}

#[test]
fn a_binding_maps_an_event_field_that_has_a_same_typed_sibling() {
    let ir = compiled(FIXTURE);

    let mut swappable = Vec::new();
    for binding in ir.bindings.values() {
        let event = ir.event(&binding.event);
        for mapping in &binding.mapping {
            if let ResolvedMappingValue::EventField { field, type_ref } = &mapping.value {
                let siblings = event
                    .fields
                    .iter()
                    .filter(|other| &other.name != field && &other.type_ref == type_ref)
                    .count();
                if siblings > 0 {
                    swappable.push(format!("{}.{field}", binding.name));
                }
            }
        }
    }

    assert!(
        !swappable.is_empty(),
        "every mapped event field is the only field of its type in its event, so a wrong mapping \
         can only be a value the target invented — never the swap between two declared sources \
         that F-WRONG-MAPPING is about"
    );
}

// ---- the matrix: every input the oracle needs, and which example carries it --------------------

/// One thing wave 4 must be able to stage, and how to tell whether an example can stage it.
///
/// The `Fault::ALL` pattern from `crates/aep-conformance/src/faulty.rs`, one layer earlier: that
/// constant makes the fault matrix a matrix rather than a list someone has to remember to extend,
/// and this one does the same for the *inputs* those faults are injected into. Seven ids from
/// design §25, three more from §50's acceptance criteria that are not faults.
type Requirement = (&'static str, &'static str, fn(&EssIr) -> bool);

/// Every requirement, with the probe that answers it.
const REQUIREMENTS: &[Requirement] = &[
    (
        "F-WRONG-EVENT",
        "an outcome emits an event, and another event exists to emit instead",
        |ir| ir.events.len() >= 2 && ir.commands.values().any(|c| c.emits().next().is_some()),
    ),
    (
        "F-REJECTION",
        "a command declares a refusal that changes nothing",
        |ir| {
            ir.commands.values().any(|command| {
                command
                    .outcomes
                    .iter()
                    .any(|outcome| outcome.error.is_some() && outcome.subject.is_none())
            })
        },
    ),
    (
        "F-ILLEGAL-TRANSITION",
        "a reachable state has a declared move that may not start there",
        |ir| {
            ir.drivers().iter().any(|(entity, driving)| {
                reachable_states(ir, entity).iter().any(|state| {
                    driving.iter().any(|driver| {
                        driver
                            .effect
                            .transition()
                            .is_some_and(|transition| !transition.from.contains(state))
                    })
                })
            })
        },
    ),
    (
        "F-DROPPED-BINDING",
        "two bindings on different events, so dropping one leaves the other green",
        |ir| ir.reactions().len() >= 2,
    ),
    (
        "F-WRONG-MAPPING",
        "a mapped event field has a same-typed sibling to be swapped with",
        |ir| {
            ir.bindings.values().any(|binding| {
                let event = ir.event(&binding.event);
                binding.mapping.iter().any(|mapping| match &mapping.value {
                    ResolvedMappingValue::EventField { field, type_ref } => event
                        .fields
                        .iter()
                        .any(|other| &other.name != field && &other.type_ref == type_ref),
                    ResolvedMappingValue::Literal { .. } => false,
                })
            })
        },
    ),
    (
        "F-VIEW-RACE",
        "a read-your-writes view holds a row at a state a command reaches",
        |ir| {
            ir.views.values().any(|view| {
                view.consistency == Consistency::ReadYourWrites
                    && selecting_states(ir, &view.source)
                        .get(&view.name.to_string())
                        .is_some_and(|states| !states.is_empty())
            })
        },
    ),
    (
        "F-EXTERNAL-OUTCOME",
        "an outcome no input decides, which a scenario injects",
        |ir| {
            ir.commands.values().any(|command| {
                command
                    .outcomes
                    .iter()
                    .any(|outcome| outcome.test_strategy == TestStrategy::InjectFault)
            })
        },
    ),
    (
        "C-BOTH-CONSISTENCIES",
        "a view at each consistency level, both able to hold a row (§50)",
        |ir| {
            [Consistency::ReadYourWrites, Consistency::Eventual]
                .iter()
                .all(|wanted| {
                    ir.views.values().any(|view| {
                        view.consistency == *wanted
                            && selecting_states(ir, &view.source)
                                .get(&view.name.to_string())
                                .is_some_and(|states| !states.is_empty())
                    })
                })
        },
    ),
    (
        "C-REFUSAL-INPUT",
        "an element synthesis must refuse a check for rather than invent one (§18, §50)",
        |ir| {
            ir.bindings
                .values()
                .any(|binding| binding.failure == Failure::Drop)
        },
    ),
    (
        "C-INVARIANT-AFTER-UPDATE",
        "an entity changed without a transition, whose invariants are then worth evaluating (§20)",
        |ir| {
            ir.drivers().iter().any(|(entity, driving)| {
                !ir.entity(entity).invariants.is_empty()
                    && driving
                        .iter()
                        .any(|driver| matches!(driver.effect, ResolvedEffect::Updates))
            })
        },
    ),
];

#[test]
fn every_input_the_oracle_needs_is_carried_by_one_of_the_examples() {
    let examples = [
        (NORMATIVE, compiled(NORMATIVE)),
        (FIXTURE, compiled(FIXTURE)),
    ];

    let uncovered: Vec<String> = REQUIREMENTS
        .iter()
        .filter(|(_, _, probe)| !examples.iter().any(|(_, ir)| probe(ir)))
        .map(|(id, what, _)| format!("{id} — {what}"))
        .collect();

    assert!(
        uncovered.is_empty(),
        "no example in this repository can stage:\n  {}\nwave 4 cannot show the matching check \
         failing, so that check cannot be trusted",
        uncovered.join("\n  ")
    );
}

#[test]
fn the_fixture_carries_something_the_normative_example_does_not() {
    let billing = compiled(NORMATIVE);
    let fixture = compiled(FIXTURE);

    let only_here: Vec<&str> = REQUIREMENTS
        .iter()
        .filter(|(_, _, probe)| !probe(&billing) && probe(&fixture))
        .map(|(id, _, _)| *id)
        .collect();

    assert!(
        !only_here.is_empty(),
        "every requirement the fixture carries is already carried by examples/{NORMATIVE}, so the \
         fixture is a second copy of the normative example and should be deleted rather than \
         maintained"
    );
}
