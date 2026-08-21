//! The conformance run against `examples/evidence-horizons-corpus/`.
//!
//! The corpus holds **43** raw annotations, and since the corpus revision of 2026-08-21
//! `expected.json` is **ground truth**: the reference implementation it was generated from was
//! fixed against this very corpus and now finds every annotation (`missed_by_reference: 0`).
//! The `reference_is_not_ground_truth` field is deliberately kept, `false`, with its reason
//! recorded beside it — the first three positions were each believed complete before the next
//! four turned up, so the standing assumption is that there is another one. Two properties bind:
//!
//! * every record must agree with `expected.json` on `date`, `horizon`, `malformed`, `state` and
//!   `days`, with nothing left over;
//! * the raw count is the **conformance target**: all 43, a divergence of zero per file, and the
//!   fenced example counted by neither side.
//!
//! `what` — the body text — is deliberately not compared: the reference historically shipped
//! records that had swallowed a neighbouring table row, and binding on body text would have
//! pinned that bug. What is asserted instead is the positive form of the same property: no
//! record's claim contains the keyword, because a claim that contains one has eaten another
//! annotation.
//!
//! Reference date **2026-09-01**, warning window **2 days**, default horizon when malformed
//! **14 days** — the three constants `expected.json` states.

use std::fs;
use std::path::PathBuf;

use aep_backend_markdown::claim::{
    horizon_growth, scan, scan_at, ClaimRecord, ClaimRejectionReason, ClaimScan, HorizonGrowth,
    DEFAULT_HORIZON_DAYS,
};
use aep_domain::time::CivilDate;

/// The corpus's reference date, so this run reads the same way in 2027.
const REFERENCE_DATE: &str = "2026-09-01";

/// The warning window `expected.json` was generated with.
const WARN_DAYS: u32 = 2;

/// The six files, with the record count a conforming implementation finds in each.
const FILES: &[(&str, usize)] = &[
    ("01-forms.md", 12),
    ("02-malformed.md", 7),
    ("03-hidden-positions.md", 7),
    ("04-classification.md", 8),
    ("05-traps.md", 4),
    ("06-reference-gaps.md", 5),
];

/// One row of `expected.json`'s `records`, on the fields that bind:
/// `(file, date, horizon in days, malformed, state, days left or over)`.
///
/// A tuple rather than a named struct so the table stays one line per record and stays readable as
/// a table.
type Expected = (&'static str, &'static str, u32, bool, &'static str, u32);

/// All 43 records of `examples/evidence-horizons-corpus/expected.json` — ground truth since the
/// corpus revision of 2026-08-21 — transcribed.
///
/// Transcribed rather than parsed: this crate has no JSON parser among its dependencies, and a
/// fixture is not worth a dependency. Regenerate with `generate_expected.py` and re-transcribe if
/// the corpus changes — a fixture whose expectations drifted from its corpus fails for the wrong
/// reason, which is worse than no fixture.
#[rustfmt::skip]
const REFERENCE_RECORDS: &[Expected] = &[
    ("01-forms.md", "2026-08-30", 7, false, "ok", 5),
    ("01-forms.md", "2026-08-29", 3, false, "expiring", 0),
    ("01-forms.md", "2026-08-28", 14, false, "ok", 10),
    ("01-forms.md", "2026-08-30", 2, false, "expiring", 0),
    ("01-forms.md", "2026-08-30", 7, false, "ok", 5),
    ("01-forms.md", "2026-08-30", 5, false, "ok", 3),
    ("01-forms.md", "2026-08-30", 14, true, "ok", 12),
    ("01-forms.md", "2026-08-30", 14, true, "ok", 12),
    ("01-forms.md", "2026-08-30", 3, false, "expiring", 1),
    ("01-forms.md", "2026-08-30", 3, false, "expiring", 1),
    ("01-forms.md", "2026-08-30", 7, false, "ok", 5),
    ("01-forms.md", "2026-08-24", 7, false, "expired", 1),
    ("02-malformed.md", "2026-08-25", 14, true, "ok", 7),
    ("02-malformed.md", "2026-08-10", 14, true, "expired", 8),
    ("02-malformed.md", "2026-08-20", 14, true, "expiring", 2),
    ("02-malformed.md", "2026-08-20", 14, true, "expiring", 2),
    ("02-malformed.md", "2026-08-20", 14, true, "expiring", 2),
    ("02-malformed.md", "2026-08-30", 3, false, "expiring", 1),
    ("02-malformed.md", "2026-07-15", 14, true, "expired", 34),
    ("03-hidden-positions.md", "2026-08-30", 7, false, "ok", 5),
    ("03-hidden-positions.md", "2026-08-29", 3, false, "expiring", 0),
    ("03-hidden-positions.md", "2026-08-28", 5, false, "expiring", 1),
    ("03-hidden-positions.md", "2026-08-30", 7, false, "ok", 5),
    ("03-hidden-positions.md", "2026-08-30", 3, false, "expiring", 1),
    ("03-hidden-positions.md", "2026-08-29", 2, false, "expired", 1),
    ("03-hidden-positions.md", "2026-08-30", 1, false, "expired", 1),
    ("04-classification.md", "2026-08-30", 7, false, "ok", 5),
    ("04-classification.md", "2026-08-25", 7, false, "expiring", 0),
    ("04-classification.md", "2026-08-24", 7, false, "expired", 1),
    ("04-classification.md", "2026-07-01", 3, false, "expired", 59),
    ("04-classification.md", "2026-08-27", 7, false, "expiring", 2),
    ("04-classification.md", "2026-08-31", 1, false, "expiring", 0),
    ("04-classification.md", "2026-08-20", 90, false, "ok", 78),
    ("04-classification.md", "2026-06-20", 30, false, "expired", 43),
    ("05-traps.md", "2026-08-25", 7, false, "expiring", 0),
    ("05-traps.md", "2026-08-30", 3, false, "expiring", 1),
    ("05-traps.md", "2026-08-30", 7, false, "ok", 5),
    ("05-traps.md", "2026-08-04", 60, false, "ok", 32),
    ("06-reference-gaps.md", "2026-08-30", 5, false, "ok", 3),
    ("06-reference-gaps.md", "2026-08-30", 7, false, "ok", 5),
    ("06-reference-gaps.md", "2026-08-29", 2, false, "expired", 1),
    ("06-reference-gaps.md", "2026-08-30", 3, false, "expiring", 1),
    ("06-reference-gaps.md", "2026-08-30", 1, false, "expired", 1),
];

/// The reference date as a value.
fn reference_date() -> CivilDate {
    CivilDate::parse(REFERENCE_DATE).expect("the reference date is a date")
}

/// The corpus directory, relative to this crate.
fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/evidence-horizons-corpus")
}

/// Reads one corpus file.
fn read(file: &str) -> String {
    let path = corpus().join("corpus").join(file);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("reading {}: {error}", path.display()))
}

/// Scans one corpus file at the reference date.
fn scan_file(file: &str) -> ClaimScan {
    scan_at(&read(file), reference_date())
}

/// The five fields that bind, as one comparable line.
fn fingerprint(record: &ClaimRecord) -> String {
    let state = record.state(reference_date(), WARN_DAYS);
    format!(
        "{} {}d malformed={} {} {}",
        record.date,
        record.horizon.as_days(),
        record.malformed,
        state.label(),
        state.days(),
    )
}

/// The same line, built from a row of `expected.json`.
fn expected_fingerprint(expected: &Expected) -> String {
    let (_, date, horizon, malformed, state, days) = *expected;
    format!("{date} {horizon}d malformed={malformed} {state} {days}")
}

/// Finds the one record in `file` written on `date`, with a horizon of `days`, about `claim`.
///
/// All three identify it: `03-hidden-positions.md` carries two records dated 2026-08-30 with a
/// seven-day horizon — the header block's and the `sprocket-api` row's — and picking either one
/// arbitrarily would make the position test pass without reaching the position.
fn record_about(file: &str, date: &str, days: u32, claim: &str) -> ClaimRecord {
    let scan = scan_file(file);
    let mut found: Vec<ClaimRecord> = scan
        .records
        .into_iter()
        .filter(|record| {
            record.date.to_string() == date
                && record.horizon.as_days() == days
                && record.claim.contains(claim)
        })
        .collect();
    assert_eq!(
        found.len(),
        1,
        "expected exactly one {date} / {days}d record about `{claim}` in {file}, found {found:#?}"
    );
    found.remove(0)
}

#[test]
fn every_annotation_in_the_corpus_is_found() {
    let mut total = 0;
    for (file, expected) in FILES {
        let scan = scan_file(file);

        assert_eq!(
            scan.records.len(),
            *expected,
            "{file}: found {:#?}",
            scan.records
                .iter()
                .map(|record| (record.line, record.claim.as_str()))
                .collect::<Vec<_>>()
        );
        total += scan.records.len();
    }

    assert_eq!(
        total, 43,
        "43 is the corpus's own count of raw annotations, and since 2026-08-21 the fixed \
         reference finds all of them too — the target did not move, the reference caught up"
    );
}

#[test]
fn every_file_reports_full_coverage_of_its_own_raw_count() {
    for (file, expected) in FILES {
        let scan = scan_file(file);

        assert_eq!(
            scan.raw_occurrences, *expected,
            "{file}: the raw count is computed without the parser and is the denominator"
        );
        assert_eq!(
            scan.divergence(),
            0,
            "{file}: a divergence is a finding, not a warning — {} raw against {} records",
            scan.raw_occurrences,
            scan.records.len()
        );
    }
}

#[test]
fn a_hyphen_separator_is_neither_a_record_nor_a_raw_occurrence() {
    let scan = scan_file("01-forms.md");

    assert!(
        !scan
            .records
            .iter()
            .any(|record| record.claim.contains("hyphen instead of em-dash")),
        "the em-dash is the convention; accepting a hyphen inflates the corpus count"
    );
    assert_eq!(
        scan.raw_occurrences, 12,
        "and it is not raw-counted either, so it is not a divergence"
    );
    let refused = scan
        .rejections
        .iter()
        .find(|rejection| rejection.text.contains("hyphen instead of em-dash"))
        .expect("it is refused with a reason rather than dropped");
    assert_eq!(refused.reason, ClaimRejectionReason::SeparatorNotEmDash);
    assert_eq!(refused.line, 84);
}

#[test]
fn the_near_miss_token_is_still_a_record_and_carries_the_flag() {
    let scan = scan_file("01-forms.md");

    let record = scan
        .records
        .iter()
        .find(|record| record.claim.contains("pallet-fork latency"))
        .expect("a space after the paren does not stop it being an annotation");

    assert!(
        record.malformed,
        "the token is deliberately strict on its left edge, so `( horizon: 5d)` did not parse"
    );
    assert_eq!(
        record.horizon.as_days(),
        DEFAULT_HORIZON_DAYS,
        "so the stated default applies, not the 5d nobody managed to write"
    );
    assert!(
        record.claim.contains("( horizon: 5d)"),
        "and the near-miss stays visible in the claim: {}",
        record.claim
    );
}

#[test]
fn every_record_the_reference_also_finds_agrees_with_expected_json() {
    for (file, _) in FILES {
        let scan = scan_file(file);
        let mut mine: Vec<String> = scan.records.iter().map(fingerprint).collect();

        for expected in REFERENCE_RECORDS.iter().filter(|row| row.0 == *file) {
            let wanted = expected_fingerprint(expected);
            let at = mine.iter().position(|found| *found == wanted);
            let Some(at) = at else {
                panic!("{file}: expected.json has `{wanted}`, which this scan does not: {mine:#?}");
            };
            mine.remove(at);
        }

        assert!(
            mine.is_empty(),
            "{file}: `expected.json` is ground truth and `missed_by_reference` is 0, so a record \
             it does not carry is a record this scan invented: {mine:#?}"
        );
    }
}

#[test]
fn no_record_swallowed_a_neighbouring_annotation() {
    // The bug the reference had (fixed upstream 2026-08-21) in `03-hidden-positions.md` record 4
    // and `06-reference-gaps.md` record 1: a table row absorbed the row below it, whose date and
    // horizon are then both lost.
    // Its signature is a claim carrying a second keyword, and it is checkable without comparing
    // any body text.
    for (file, _) in FILES {
        for record in scan_file(file).records {
            assert!(
                !record.claim.contains("Verify:"),
                "{file}:{}: this claim has eaten another annotation: {}",
                record.line,
                record.claim
            );
            // The same rule one boundary earlier: a claim written in a table cell ends at the
            // cell, so a record that carries a `|` has read past the annotation it belongs to.
            assert!(
                !record.claim.contains('|'),
                "{file}:{}: this claim ran past its cell: {}",
                record.line,
                record.claim
            );
        }
    }
}

#[test]
fn the_positions_the_reference_is_blind_to_are_found() {
    // Six well-formed annotations in the two files. When this test was written the reference
    // found exactly one of them; it finds all six since 2026-08-21, and the cases stay because
    // they are the regression that stops the positions coming back.
    let cases = [
        (
            "03-hidden-positions.md",
            "2026-08-30",
            7,
            "ok",
            5,
            "values file",
        ),
        (
            "03-hidden-positions.md",
            "2026-08-29",
            2,
            "expired",
            1,
            "row counts match",
        ),
        (
            "06-reference-gaps.md",
            "2026-08-30",
            5,
            "ok",
            3,
            "the open review",
        ),
        (
            "06-reference-gaps.md",
            "2026-08-30",
            7,
            "ok",
            5,
            "pin read from the running image.",
        ),
        (
            "06-reference-gaps.md",
            "2026-08-29",
            2,
            "expired",
            1,
            "redeploys daily",
        ),
        (
            "06-reference-gaps.md",
            "2026-08-30",
            3,
            "expiring",
            1,
            "the escapement rework is enabled",
        ),
    ];

    for (file, date, days, state, remaining, claim) in cases {
        let record = record_about(file, date, days, claim);
        let found = record.state(reference_date(), WARN_DAYS);

        assert!(!record.malformed, "{file}: {date}/{days}d is well-formed");
        assert_eq!(found.label(), state, "{file}: {date}/{days}d");
        assert_eq!(found.days(), remaining, "{file}: {date}/{days}d");
    }

    // Position 6 ends at its closing backtick, so the claim is what the reader sees inside the
    // span. A trailing backtick left in it is the span never having been closed.
    let backticked = record_about("06-reference-gaps.md", "2026-08-30", 3, "escapement rework");
    assert_eq!(
        backticked.claim,
        "the escapement rework is enabled in atlas; read from the deployment."
    );
}

#[test]
fn a_backticked_annotation_after_prose_is_found_and_a_fenced_one_is_not() {
    // The corpus revision of 2026-08-21 added position 7 and its inverse together. The mid-line
    // backtick is a real claim — the live instance behind it had a one-day horizon and was
    // already stale — while an annotation inside a fenced code block is a document showing a
    // reader the convention, and must be counted by neither the parser nor the raw denominator:
    // otherwise every document that explains the convention reports a permanent coverage gap.
    let record = record_about("06-reference-gaps.md", "2026-08-30", 1, "mainspring 3.0.1");
    let state = record.state(reference_date(), WARN_DAYS);
    assert!(!record.malformed, "position 7 is well-formed");
    assert_eq!(
        state.label(),
        "expired",
        "a 1d horizon observed 2026-08-30, read 2026-09-01"
    );

    let scan = scan_file("06-reference-gaps.md");
    assert!(
        scan.records
            .iter()
            .all(|found| !found.claim.contains("EXAMPLE")),
        "the fenced example was parsed as a claim: {:#?}",
        scan.records
    );
    assert_eq!(
        scan.raw_occurrences, 5,
        "the fenced example is outside the denominator too, or the file reports a permanent, \
         unfixable divergence"
    );
    assert_eq!(scan.divergence(), 0);
}

#[test]
fn the_two_traps_classify_ok_and_nothing_is_invented() {
    let scan = scan_file("05-traps.md");

    assert_eq!(scan.records.len(), 4);
    assert!(
        scan.rejections.is_empty(),
        "neither trap is something a parser may refuse: {:#?}",
        scan.rejections
    );
    for record in &scan.records {
        let state = record.state(reference_date(), WARN_DAYS);
        assert!(
            !state.is_expired(),
            "05-traps.md:{}: a horizon is a volatility guess, and a claim can be false inside it \
             — but the clock says {state} and a conforming implementation must not invent a \
             contradiction check it cannot ground",
            record.line
        );
        assert!(!record.malformed, "all four are well-formed");
    }
}

#[test]
fn a_horizon_that_grew_while_its_observation_date_stood_still_is_reported() {
    // The fixture cannot supply `before` on its own: `05-traps.md` holds the same claim at
    // 2026-08-30/7d and at 2026-08-04/60d, so neither ordering is "the horizon grew while the date
    // stood still". The earlier reading of the forbidden row is what is missing, so the test
    // builds it — the forbidden row's date with the re-checked row's horizon — and uses the
    // fixture's own row as the later one.
    let subject = "the grommet index row count matches the primary store.";
    let forbidden = record_about("05-traps.md", "2026-08-04", 60, subject);
    let rechecked = record_about("05-traps.md", "2026-08-30", 7, subject);
    assert_eq!(
        forbidden.claim, rechecked.claim,
        "the two rows are the same subject refreshed the two ways"
    );

    let before = scan(&format!(
        "Verify: {} — {} (horizon: {})\n",
        forbidden.date, forbidden.claim, rechecked.horizon
    ));
    let after = scan(&format!("{}\n", forbidden.render()));
    let refreshed = scan(&format!("{}\n", rechecked.render()));

    let flagged = horizon_growth(&before, &after);
    let ignored = horizon_growth(&before, &refreshed);

    assert_eq!(flagged.len(), 1, "one pair, and it is the forbidden one");
    let HorizonGrowth::Grew {
        claim,
        date,
        before: was,
        after: now,
        ..
    } = &flagged[0]
    else {
        panic!("expected a growth, found {:#?}", flagged[0]);
    };
    assert_eq!(claim, &forbidden.claim);
    assert_eq!(date.to_string(), "2026-08-04");
    assert_eq!(was.as_days(), 7);
    assert_eq!(now.as_days(), 60);
    assert!(
        ignored.is_empty(),
        "observing again and writing a new date is the one correct refresh, and must not be \
         flagged: {ignored:#?}"
    );
}

#[test]
fn every_well_formed_record_survives_a_round_trip() {
    let mut round_tripped = 0;
    for (file, _) in FILES {
        for record in scan_file(file).records {
            if record.malformed {
                continue;
            }
            let again = scan(&record.render());

            assert_eq!(
                again.records.len(),
                1,
                "{file}:{}: rendering produced {} records from one: {}",
                record.line,
                again.records.len(),
                record.render()
            );
            let round = &again.records[0];
            assert_eq!(round.date, record.date, "{file}:{}", record.line);
            assert_eq!(round.horizon, record.horizon, "{file}:{}", record.line);
            assert_eq!(round.malformed, record.malformed, "{file}:{}", record.line);
            assert_eq!(round.claim, record.claim, "{file}:{}", record.line);
            round_tripped += 1;
        }
    }

    assert_eq!(
        round_tripped, 35,
        "35 of the 43 are well-formed; the other 8 are the normalisation refusal below"
    );
}

#[test]
fn a_malformed_record_refuses_to_normalise() {
    // Not a round trip: rendering a malformed record returns its source, so `scan(render(r))`
    // would be `scan(source)` and would hold whatever the parser did. The property that carries
    // weight is the refusal — the 14d default must not be written into the document as though the
    // author had chosen it, which would turn a carried flag into an inferred one. A malformed span
    // may be more than one line: `02-malformed.md:33-34` is one.
    let mut malformed = 0;
    for (file, _) in FILES {
        for record in scan_file(file).records {
            if !record.malformed {
                continue;
            }
            let rendered = record.render();

            assert_eq!(
                rendered, record.source,
                "{file}:{}: a malformed record renders its source span verbatim",
                record.line
            );
            assert!(
                !rendered.contains("(horizon: 14d)"),
                "{file}:{}: the default must not be written in as a choice: {rendered}",
                record.line
            );
            assert_ne!(
                rendered,
                format!(
                    "Verify: {} — {} (horizon: {})",
                    record.date, record.claim, record.horizon
                ),
                "{file}:{}: and the canonical form is exactly what is refused",
                record.line
            );
            malformed += 1;
        }
    }

    assert_eq!(
        malformed, 8,
        "the corpus carries 8 malformed records, and `expected.json` counts the same 8"
    );
}

#[test]
fn an_annotation_dated_after_the_reference_date_is_refused_rather_than_recorded() {
    // The corpus has no future dates, so this one is written here. A planned re-check is a
    // different object from a decaying observation: read as an observation it has a negative age,
    // which inflates its remaining horizon and makes it the freshest record in the corpus.
    let text =
        "Verify: 2026-09-08 — the mainspring migration will have been applied. (horizon: 7d)\n";

    let scan = scan_at(text, reference_date());

    assert!(
        scan.records.is_empty(),
        "a scheduled check is not a record: {:#?}",
        scan.records
    );
    assert_eq!(
        scan.rejections.len(),
        1,
        "it is refused, by one comparison, with a reason"
    );
    assert_eq!(
        scan.rejections[0].reason,
        ClaimRejectionReason::FutureObservation
    );
    assert_eq!(
        scan.divergence(),
        1,
        "and the refusal shows up in the coverage claim rather than vanishing from it"
    );
}
