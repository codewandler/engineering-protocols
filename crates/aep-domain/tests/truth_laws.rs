//! Invariant 5's algebra, asserted as laws rather than as table rows: `Truth` is a Kleene logic.
//!
//! The inline tests on `Truth` check the tables. What the tables cannot say is that the algebra
//! *composes* — that any predicate tree built from `and`, `or` and `not` behaves the same however
//! it is associated, distributed or negated, which is what `Predicate::evaluate` relies on when it
//! folds a tree in its own order. These are the laws of strong Kleene three-valued logic, checked
//! over generated triples; with three values a triple has 27 shapes, so the default 256 cases
//! saturate the space many times over — the generator earns its keep as shrinkage and as the
//! shape the phase-2 property work builds on (the property-testing decision, wave 3.5
//! reconciliation page, § "The property-based work, and where it sits").
//!
//! # Determinism
//!
//! The gate must not be flaky (invariant 9's spirit), so the runner is seeded with a fixed value
//! below and the sequence of cases is the same on every run. To explore beyond the committed
//! sequence locally, raise `PROPTEST_CASES` (the fixed seed extends deterministically) or edit the
//! seed to `RngSeed::Random` for one session; commit neither.

use aep_domain::predicate::Truth;
use proptest::prelude::*;
use proptest::test_runner::RngSeed;

/// The fixed-seed configuration every property in this file runs under.
fn seeded() -> ProptestConfig {
    ProptestConfig {
        rng_seed: RngSeed::Fixed(0x5eed_0001),
        ..ProptestConfig::default()
    }
}

/// One of the three truth values.
fn truth() -> impl Strategy<Value = Truth> {
    prop_oneof![Just(Truth::True), Just(Truth::False), Just(Truth::Unknown)]
}

proptest! {
    #![proptest_config(seeded())]

    #[test]
    fn negation_is_an_involution(a in truth()) {
        prop_assert_eq!(a.not().not(), a);
    }

    #[test]
    fn de_morgan_holds_in_both_directions(a in truth(), b in truth()) {
        prop_assert_eq!(a.and(b).not(), a.not().or(b.not()));
        prop_assert_eq!(a.or(b).not(), a.not().and(b.not()));
    }

    #[test]
    fn conjunction_and_disjunction_commute(a in truth(), b in truth()) {
        prop_assert_eq!(a.and(b), b.and(a));
        prop_assert_eq!(a.or(b), b.or(a));
    }

    #[test]
    fn conjunction_and_disjunction_associate(a in truth(), b in truth(), c in truth()) {
        prop_assert_eq!(a.and(b).and(c), a.and(b.and(c)));
        prop_assert_eq!(a.or(b).or(c), a.or(b.or(c)));
    }

    #[test]
    fn each_operation_distributes_over_the_other(a in truth(), b in truth(), c in truth()) {
        prop_assert_eq!(a.and(b.or(c)), a.and(b).or(a.and(c)));
        prop_assert_eq!(a.or(b.and(c)), a.or(b).and(a.or(c)));
    }

    #[test]
    fn identities_annihilators_idempotence_and_absorption(a in truth(), b in truth()) {
        prop_assert_eq!(a.and(Truth::True), a);
        prop_assert_eq!(a.or(Truth::False), a);
        prop_assert_eq!(a.and(Truth::False), Truth::False);
        prop_assert_eq!(a.or(Truth::True), Truth::True);
        prop_assert_eq!(a.and(a), a);
        prop_assert_eq!(a.or(a), a);
        prop_assert_eq!(a.and(a.or(b)), a);
        prop_assert_eq!(a.or(a.and(b)), a);
    }

    /// Invariant 5 itself, as an algebraic fact: only `True` permits, and composition cannot
    /// manufacture permission. `Unknown` is not `False`, but neither of them satisfies.
    #[test]
    fn only_true_permits_and_composition_cannot_widen_it(a in truth(), b in truth()) {
        prop_assert_eq!(a.and(b).is_satisfied(), a.is_satisfied() && b.is_satisfied());
        prop_assert_eq!(a.or(b).is_satisfied(), a.is_satisfied() || b.is_satisfied());
        prop_assert!(!Truth::Unknown.is_satisfied());
        prop_assert_ne!(Truth::Unknown, Truth::False);
    }
}
