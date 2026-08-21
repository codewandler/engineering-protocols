//! `Verify:` annotations, read out of human-written markdown.
//!
//! An early adopter keeps a corpus of dated claims in ordinary documents, one line each:
//!
//! ```text
//! Verify: 2026-08-30 — sprocket-api is running image v4.2.1 in atlas. (horizon: 7d)
//!         ^ the observation   ^ the claim                              ^ the requirement
//! ```
//!
//! One line carries both halves of the split
//! `docs/design/evidence-horizons-design-v0.1.md` § 3 makes in the domain: the date is *when
//! somebody looked* ([`ObservedAt`]), and the token is *how often this must be re-checked*
//! ([`Horizon`]) — the requirement's half, not the record's. A scanned horizon binds this
//! scanner's report and travels no further, because an evidence record has no field for it to
//! travel in.
//!
//! # Why this is not a line-anchored parser
//!
//! Six positions have been found where an annotation is present, correct, legible to a human, and
//! invisible to a parser whose idea of *a line* disagrees with the document's — three of them in
//! production, over months, each after the previous fix was believed complete:
//!
//! | # | position | consequence when missed |
//! |---|---|---|
//! | 1 | the token on a wrapped continuation line | watched on the default clock, not the stated one |
//! | 2 | inside a `>` quote block, including a file's leading header block | never watched |
//! | 3 | after `<br>` inside a table cell | never watched |
//! | 4 | ending a table cell with no `<br>` at all | never watched |
//! | 5 | the second of two consecutive `<br>` rows, absorbed into the first's body | date **and** horizon lost |
//! | 6 | wrapped in inline-code backticks | never watched |
//! | 7 | in inline-code backticks **mid-line, after prose** | never watched |
//!
//! So a document is read as physical lines *and* as the smaller pieces a reader sees inside one:
//! a quote marker is scaffolding, a `<br>` and a cell boundary each start a new line, and an
//! inline-code span ends one.
//!
//! One position runs the other way: an annotation inside a **fenced code block** is a document
//! showing a reader what the convention looks like, and is excluded from parsing *and* from
//! [`raw_occurrences`] — otherwise every document that explains the convention reports a
//! permanent, unfixable coverage gap. Inline backticks cannot carry that meaning, because
//! positions 6 and 7 are real claims written in them; the rule is one-directional — fence it if
//! you are illustrating, and anything else parses.
//!
//! # Where a claim's body ends
//!
//! A body absorbs wrapped continuation lines, which is position 1 above. It stops at each of
//! these, and the list is binding:
//!
//! * another annotation keyword — `Verify:` or `Due:`;
//! * a heading;
//! * a list bullet;
//! * a blank line;
//! * a bare `>` line;
//! * a new table row;
//! * the end of the table cell it started in.
//!
//! A `Last updated:` line is **not** a stop: it is ordinary body text, and the header block of
//! `examples/evidence-horizons-corpus/corpus/03-hidden-positions.md` is the case that says so.
//! Nothing binds on a claim's exact prose, so absorbing it costs nothing; inventing a stop for
//! every `Word:` line would eventually eat an annotation.
//!
//! # The coverage claim
//!
//! Handling six positions is not the design; [`ClaimScan::divergence`] is. [`raw_occurrences`]
//! counts what a human would call an annotation **without using this parser**, and the difference
//! against [`ClaimScan::records`] is a number the scan reports about itself. That one comparison
//! is what surfaced 15 unwatched annotations in 160 on a live corpus — 9.4%, inside a gate whose
//! entire job was making an unchecked claim visible. A divergence is a finding, never a silent
//! drop.
//!
//! # Two things this deliberately does not do
//!
//! It does not infer [`ClaimRecord::malformed`] from the horizon value. The default applied to an
//! unparseable token is 14 days, and 14 days is also the second most commonly *chosen* horizon in
//! the source corpus (33 of 167 tokens), so recovering the flag by testing the value would mark a
//! third of a healthy corpus as decayed convention. The flag is carried.
//!
//! It does not read a clock. [`scan_at`] takes the reference date as an argument, which is what
//! keeps this crate's no-clock property (see the crate docs) and what makes a corpus test
//! reproducible in 2027.

use std::collections::BTreeMap;
use std::fmt;

use aep_domain::time::{CivilDate, Horizon, ObservedAt};

/// The horizon a record is given when its token is absent or does not parse: fourteen days.
///
/// Stated rather than implied, because the alternative — treating *no token* as *no expiry* — is
/// how the convention decays into undated prose one annotation at a time.
pub const DEFAULT_HORIZON_DAYS: u32 = 14;

/// The keyword that opens an annotation.
const KEYWORD: &str = "Verify:";

/// The convention's other annotation keyword. Nothing here parses one; a body stops at it, so a
/// scheduled action is not swallowed into an observation's claim and lost from its own gate.
const OTHER_KEYWORD: &str = "Due:";

/// The separator, which is an em-dash and nothing else. A hyphen is not an annotation.
const SEPARATOR: char = '—';

/// The keyword inside the horizon token, matched case-insensitively and flush against the `(`.
const TOKEN_KEYWORD: &str = "horizon";

/// The written length of `YYYY-MM-DD`.
const DATE_LEN: usize = 10;

/// The default horizon as a [`Horizon`].
fn default_horizon() -> Horizon {
    Horizon::days(DEFAULT_HORIZON_DAYS).expect("14 days is inside the accepted horizon range")
}

/// Where a claim stands against its horizon at some reference date.
///
/// `Expiring` is a **report** state, not a gate state: it permits everything `Ok` permits and
/// exists for a human reading a list. Only `Expired` changes a truth value, and it changes it to
/// `Unknown` — never to `False`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimState {
    /// Inside its horizon, with more than the warning window left.
    Ok {
        /// Whole days before the horizon runs out.
        remaining_days: u32,
    },
    /// Inside its horizon and inside the warning window.
    Expiring {
        /// Whole days before the horizon runs out; zero on the last day it covers.
        remaining_days: u32,
    },
    /// Past its horizon. Nobody has looked since, so the fact it fed reads `Unknown`.
    Expired {
        /// Whole days past the horizon.
        overrun_days: u32,
    },
}

impl ClaimState {
    /// The corpus's own vocabulary for this state: `ok`, `expiring` or `expired`.
    pub fn label(self) -> &'static str {
        match self {
            Self::Ok { .. } => "ok",
            Self::Expiring { .. } => "expiring",
            Self::Expired { .. } => "expired",
        }
    }

    /// Days left for `Ok` and `Expiring`, days over for `Expired`.
    pub fn days(self) -> u32 {
        match self {
            Self::Ok { remaining_days } | Self::Expiring { remaining_days } => remaining_days,
            Self::Expired { overrun_days } => overrun_days,
        }
    }

    /// Whether the observation behind this claim has gone past its horizon.
    pub fn is_expired(self) -> bool {
        matches!(self, Self::Expired { .. })
    }
}

impl fmt::Display for ClaimState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ok { remaining_days } => write!(f, "ok, {remaining_days}d left"),
            Self::Expiring { remaining_days } => write!(f, "expiring, {remaining_days}d left"),
            Self::Expired { overrun_days } => write!(f, "expired, {overrun_days}d over"),
        }
    }
}

/// One annotation, as the document wrote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimRecord {
    /// What was observed, with the horizon token removed and whitespace collapsed.
    pub claim: String,
    /// When somebody looked, as the protocol's own type.
    pub observed_at: ObservedAt,
    /// The same instant as the document wrote it, which is a day and not an instant.
    pub date: CivilDate,
    /// How long the observation is worth something. [`DEFAULT_HORIZON_DAYS`] when `malformed`.
    pub horizon: Horizon,
    /// Whether the horizon was stated by the author or supplied by the default.
    ///
    /// Carried, never inferred: a record may legitimately have chosen 14 days.
    pub malformed: bool,
    /// The annotation's source span, from the keyword to the end of its body, less the document
    /// scaffolding that is not part of it (quote markers, cell boundaries, an enclosing
    /// inline-code span).
    pub source: String,
    /// The one-based physical line the keyword sits on.
    pub line: usize,
}

impl ClaimRecord {
    /// Where this claim stands at `at`, with a warning window of `warn_days`.
    ///
    /// `age == horizon` is **not** expired. The corpus states the cost of the other choice: an
    /// off-by-one here fires the gate a day early on every record in a corpus, which is how a gate
    /// gets muted and then deleted.
    ///
    /// An observation dated after `at` has a negative age, so it reports more days remaining than
    /// its horizon allows — the pathology [`ClaimRejectionReason::FutureObservation`] exists to
    /// keep out of a scan in the first place.
    pub fn state(&self, at: CivilDate, warn_days: u32) -> ClaimState {
        let age = at.days_since(self.date);
        let remaining = i64::from(self.horizon.as_days()) - age;
        if remaining < 0 {
            return ClaimState::Expired {
                overrun_days: u32::try_from(-remaining).unwrap_or(u32::MAX),
            };
        }
        let remaining_days = u32::try_from(remaining).unwrap_or(u32::MAX);
        if remaining_days <= warn_days {
            ClaimState::Expiring { remaining_days }
        } else {
            ClaimState::Ok { remaining_days }
        }
    }

    /// The canonical one line for a well-formed record, and the **source span verbatim** for a
    /// malformed one.
    ///
    /// The refusal to normalise is the point. Rendering a malformed record canonically would write
    /// its 14-day default into the document as though the author had chosen it, turning a carried
    /// flag into an inferred one and losing the only evidence that the convention is decaying
    /// there.
    pub fn render(&self) -> String {
        if self.malformed {
            return self.source.clone();
        }
        format!(
            "{KEYWORD} {date} {SEPARATOR} {claim} ({TOKEN_KEYWORD}: {horizon})",
            date = self.date,
            claim = self.claim,
            horizon = self.horizon,
        )
    }
}

/// Why a line that looked like an annotation did not become one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClaimRejectionReason {
    /// The separator was not an em-dash.
    ///
    /// Deliberately fatal rather than tolerated: accepting a hyphen inflates the corpus count and
    /// hides a real malformed line in the noise.
    SeparatorNotEmDash,
    /// The date has the shape of a date and is not one — `2026-02-30`, say.
    ImpossibleDate,
    /// The observation is dated after the reference date.
    ///
    /// A planned re-check is a different object from a decaying observation. Under a one-field
    /// convention a scheduled-but-never-performed check reads as the *freshest* record there is,
    /// because a negative age inflates the remaining horizon — and the model can no longer answer
    /// *has anyone ever looked at this?*. One comparison makes that conflation unwritable.
    FutureObservation,
}

impl fmt::Display for ClaimRejectionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reason = match self {
            Self::SeparatorNotEmDash => "the separator is not an em-dash",
            Self::ImpossibleDate => "the calendar has no such date",
            Self::FutureObservation => "the observation is dated after the reference date",
        };
        f.write_str(reason)
    }
}

/// An annotation-shaped line the scan refused, with the reason it refused it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimRejection {
    /// The one-based physical line the keyword sits on.
    pub line: usize,
    /// The refused text, from the keyword to the end of its line or cell.
    pub text: String,
    /// Why it was refused.
    pub reason: ClaimRejectionReason,
}

/// The result of reading one document: what was found, what was refused, and what was there.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClaimScan {
    /// The annotations, in document order.
    pub records: Vec<ClaimRecord>,
    /// Annotation-shaped occurrences counted by [`raw_occurrences`], without this parser.
    pub raw_occurrences: usize,
    /// Occurrences refused, each with a reason.
    pub rejections: Vec<ClaimRejection>,
}

impl ClaimScan {
    /// `raw_occurrences - records.len()`. Non-zero is a finding, not a warning.
    ///
    /// A rejection explains a divergence rather than cancelling it: the scan saw something a human
    /// would call an annotation and produced no record for it, and that is exactly what the number
    /// is for. The one shape that is neither — a hyphen separator — is not raw-counted either, so
    /// a document full of them still reports zero.
    pub fn divergence(&self) -> usize {
        self.raw_occurrences.saturating_sub(self.records.len())
    }
}

/// What the horizon-growth diagnostic found for one claim.
///
/// There is exactly one correct way to refresh a claim: observe it again and write a new date.
/// Growing the horizon instead produces a record that reports as fresh and has not been looked at
/// since the original reading, and no parser can tell the two apart from a single document — which
/// is why this takes two readings.
///
/// Two variants rather than one, because a diagnostic that cannot say *I could not tell* pairs
/// readings arbitrarily and reports the arbitrary pairing as a finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HorizonGrowth {
    /// The horizon grew while the observation date stood still. This is the finding.
    Grew {
        /// The claim text both readings share.
        claim: String,
        /// The observation date that did not move.
        date: CivilDate,
        /// The same date as the protocol's type.
        observed_at: ObservedAt,
        /// The horizon in the earlier reading.
        before: Horizon,
        /// The horizon in the later reading, which is longer.
        after: Horizon,
    },
    /// One reading holds the same claim at the same observation date more than once, so which
    /// record grew into which is not decidable from the two documents.
    Ambiguous {
        /// The claim text the readings share.
        claim: String,
        /// The observation date they share.
        date: CivilDate,
        /// The same date as the protocol's type.
        observed_at: ObservedAt,
        /// How many records carry this claim and date in the earlier reading.
        before_readings: usize,
        /// How many carry it in the later reading.
        after_readings: usize,
    },
}

impl HorizonGrowth {
    /// The claim this finding is about.
    pub fn claim(&self) -> &str {
        match self {
            Self::Grew { claim, .. } | Self::Ambiguous { claim, .. } => claim,
        }
    }

    /// Whether this is a growth rather than a report that the pairing could not be decided.
    pub fn is_growth(&self) -> bool {
        matches!(self, Self::Grew { .. })
    }
}

/// Reads a document with no reference date, so no observation can be refused for being in the
/// future. Use [`scan_at`] wherever a reference date exists.
pub fn scan(text: &str) -> ClaimScan {
    scan_inner(text, None)
}

/// Reads a document as of `now`, refusing any observation dated after it.
pub fn scan_at(text: &str, now: CivilDate) -> ClaimScan {
    scan_inner(text, Some(now))
}

/// Counts annotation-shaped occurrences — `Verify:`, a written date, an em-dash — **without the
/// parser**.
///
/// A lower bound on what a human would call an annotation, and the denominator of the scan's
/// coverage claim. It is deliberately the crudest thing that works: the moment it shares code with
/// the parser it stops being independent evidence and starts agreeing with it by construction.
pub fn raw_occurrences(text: &str) -> usize {
    // Fence-stripping is implemented here *again* rather than shared with the parser, for the
    // same reason the function exists at all: a denominator that borrows the parser's reading
    // agrees with it by construction.
    let mut fenced = false;
    let mut kept = String::with_capacity(text.len());
    for line in text.lines() {
        let bare = line
            .trim_start()
            .trim_start_matches(['>', ' '])
            .trim_start();
        if bare.starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if !fenced {
            kept.push_str(line);
            kept.push('\n');
        }
    }
    let text = kept.as_str();
    let mut count = 0;
    for (at, _) in text.match_indices(KEYWORD) {
        let rest = text[at + KEYWORD.len()..].trim_start();
        let Some(date) = rest.get(..DATE_LEN) else {
            continue;
        };
        if !is_iso_shaped(date) {
            continue;
        }
        if rest[DATE_LEN..].trim_start().starts_with(SEPARATOR) {
            count += 1;
        }
    }
    count
}

/// Pairs the two readings on `(claim text, observation date)` and reports each pair whose horizon
/// grew while its observation date did not.
///
/// Matching on the claim rather than on a position is what makes it usable across two revisions of
/// a document that has been edited in between. The **date is half the key**, not a field to
/// compare afterwards: two records with the same text and different dates are two different facts,
/// and pairing them would report every ordinary re-check as growth. Which is why the fixture's own
/// trap-2 pair cannot be handed to this function as it stands — `05-traps.md` holds
/// `2026-08-30 (horizon: 7d)` beside `2026-08-04 (horizon: 60d)`, and neither ordering has a
/// standing date.
pub fn horizon_growth(before: &ClaimScan, after: &ClaimScan) -> Vec<HorizonGrowth> {
    let earlier = readings_by_claim(before);
    let later = readings_by_claim(after);
    let mut findings = Vec::new();
    for (key, before_readings) in &earlier {
        let Some(after_readings) = later.get(key) else {
            continue;
        };
        let (claim, observed_at) = *key;
        if before_readings.len() > 1 || after_readings.len() > 1 {
            findings.push(HorizonGrowth::Ambiguous {
                claim: claim.to_string(),
                date: before_readings[0].date,
                observed_at,
                before_readings: before_readings.len(),
                after_readings: after_readings.len(),
            });
            continue;
        }
        let (one, two) = (before_readings[0], after_readings[0]);
        if two.horizon > one.horizon {
            findings.push(HorizonGrowth::Grew {
                claim: claim.to_string(),
                date: one.date,
                observed_at,
                before: one.horizon,
                after: two.horizon,
            });
        }
    }
    findings
}

/// Groups a reading's records by the identity of the fact they state.
///
/// A `BTreeMap` rather than a `HashMap` so the findings come out in the same order every run —
/// invariant 9.
fn readings_by_claim(scan: &ClaimScan) -> BTreeMap<(&str, ObservedAt), Vec<&ClaimRecord>> {
    let mut readings: BTreeMap<(&str, ObservedAt), Vec<&ClaimRecord>> = BTreeMap::new();
    for record in &scan.records {
        readings
            .entry((record.claim.as_str(), record.observed_at))
            .or_default()
            .push(record);
    }
    readings
}

/// One physical line, stripped of the scaffolding a reader ignores.
struct Line<'a> {
    /// The one-based line number in the document.
    number: usize,
    /// Whether the line sat inside a `>` quote block.
    quoted: bool,
    /// Whether a body running into this line must stop at it.
    stops_a_body: bool,
    /// The pieces a reader sees as separate lines inside this one.
    segments: Vec<Segment<'a>>,
}

/// A stretch of one physical line that a reader sees as a line of its own.
struct Segment<'a> {
    /// The text, without the boundary that ended it.
    text: &'a str,
    /// Whether it ended at a `<br>` or a cell boundary rather than at the line's end.
    bounded: bool,
}

/// What follows the keyword.
struct Head<'a> {
    /// The written date, still unvalidated.
    date: &'a str,
    /// Where the claim starts, relative to the slice this was read from.
    body_start: usize,
    /// Whether the separator was the em-dash the convention requires.
    em_dash: bool,
}

fn scan_inner(text: &str, now: Option<CivilDate>) -> ClaimScan {
    let lines = read_lines(text);
    let mut scan = ClaimScan {
        records: Vec::new(),
        raw_occurrences: raw_occurrences(text),
        rejections: Vec::new(),
    };
    for index in 0..lines.len() {
        for segment in &lines[index].segments {
            scan_segment(&lines, index, segment, now, &mut scan);
        }
    }
    scan
}

/// Reads every annotation in one segment.
fn scan_segment(
    lines: &[Line<'_>],
    index: usize,
    segment: &Segment<'_>,
    now: Option<CivilDate>,
    scan: &mut ClaimScan,
) {
    let text = segment.text;
    let mut cursor = 0;
    while let Some(offset) = text[cursor..].find(KEYWORD) {
        let start = cursor + offset;
        let after_keyword = start + KEYWORD.len();
        cursor = after_keyword;
        let limit = span_limit(text, start, after_keyword);
        if after_keyword > limit {
            continue;
        }
        let Some(head) = read_head(&text[after_keyword..limit]) else {
            continue;
        };
        let refused = |reason| ClaimRejection {
            line: lines[index].number,
            text: text[start..limit].trim().to_string(),
            reason,
        };
        if !head.em_dash {
            scan.rejections
                .push(refused(ClaimRejectionReason::SeparatorNotEmDash));
            continue;
        }
        let Ok(date) = CivilDate::parse(head.date) else {
            scan.rejections
                .push(refused(ClaimRejectionReason::ImpossibleDate));
            continue;
        };
        if now.is_some_and(|reference| date > reference) {
            scan.rejections
                .push(refused(ClaimRejectionReason::FutureObservation));
            continue;
        }
        let body_start = after_keyword + head.body_start;
        let gathered = assemble(lines, index, segment, start, body_start, limit);
        cursor = gathered.end;
        scan.records
            .push(record(date, &gathered, lines[index].number));
    }
}

/// An annotation's text, gathered across whatever continuation lines belong to it.
struct Span {
    /// The source span, less scaffolding.
    source: String,
    /// The claim and its token, joined into one line.
    body: String,
    /// Where the body ended in the segment it started in.
    end: usize,
}

/// Gathers the body, absorbing continuation lines until something stops it.
fn assemble(
    lines: &[Line<'_>],
    index: usize,
    segment: &Segment<'_>,
    start: usize,
    body_start: usize,
    limit: usize,
) -> Span {
    let text = segment.text;
    let end = body_stop(text, body_start, limit);
    let mut source = vec![text[start..end].trim_end()];
    let mut body = vec![text[body_start..end].trim()];
    let mut open = end == text.len() && !segment.bounded;
    let mut next = index + 1;
    while open {
        let Some(line) = lines.get(next) else {
            break;
        };
        if line.stops_a_body || line.quoted != lines[index].quoted {
            break;
        }
        let Some(first) = line.segments.first() else {
            break;
        };
        let stop = body_stop(first.text, 0, first.text.len());
        let piece = first.text[..stop].trim();
        source.push(piece);
        body.push(piece);
        open = stop == first.text.len() && !first.bounded;
        next += 1;
    }
    Span {
        source: source.join("\n"),
        body: body.join(" "),
        end,
    }
}

/// Turns a gathered span into a record, applying the default when no token parses.
fn record(date: CivilDate, span: &Span, line: usize) -> ClaimRecord {
    let (horizon, malformed, claim) = match find_token(&span.body) {
        Some((from, to, horizon)) => {
            let mut without = String::with_capacity(span.body.len());
            without.push_str(&span.body[..from]);
            without.push_str(&span.body[to..]);
            (horizon, false, collapse(&without))
        }
        None => (default_horizon(), true, collapse(&span.body)),
    };
    ClaimRecord {
        claim,
        observed_at: ObservedAt::new(date.to_timestamp()),
        date,
        horizon,
        malformed,
        source: span.source.clone(),
        line,
    }
}

/// Splits a document into lines, each stripped of its quote markers and cut at its boundaries.
fn read_lines(text: &str) -> Vec<Line<'_>> {
    let mut fenced = false;
    text.lines()
        .enumerate()
        .map(|(index, raw)| {
            let (stripped, quoted) = strip_quote(raw);
            let marker = stripped.trim().starts_with("```");
            let excluded = fenced || marker;
            if marker {
                fenced = !fenced;
            }
            Line {
                number: index + 1,
                quoted,
                // A fence stops a body on the way in, and nothing inside one may start or
                // continue a claim: a fenced annotation is an illustration, not an assertion.
                stops_a_body: excluded || is_stop_line(stripped),
                segments: if excluded {
                    Vec::new()
                } else {
                    split_segments(stripped)
                },
            }
        })
        .collect()
}

/// Drops a line's `>` markers, reporting whether there were any.
fn strip_quote(line: &str) -> (&str, bool) {
    let mut rest = line.trim_start();
    let mut quoted = false;
    while let Some(after) = rest.strip_prefix('>') {
        quoted = true;
        rest = after.trim_start();
    }
    (rest, quoted)
}

/// Whether a body running into this line must stop at it.
fn is_stop_line(stripped: &str) -> bool {
    let text = stripped.trim();
    text.is_empty()
        || text.starts_with('#')
        || text.starts_with('|')
        || text.starts_with("```")
        || text.starts_with(KEYWORD)
        || text.starts_with(OTHER_KEYWORD)
        || is_bullet(text)
}

/// Whether a line opens a list item.
fn is_bullet(text: &str) -> bool {
    if ["- ", "* ", "+ "].iter().any(|mark| text.starts_with(mark)) {
        return true;
    }
    let digits = text.bytes().take_while(u8::is_ascii_digit).count();
    digits > 0
        && matches!(text.as_bytes().get(digits), Some(b'.' | b')'))
        && text.as_bytes().get(digits + 1) == Some(&b' ')
}

/// Cuts a line at every boundary a reader treats as a line break.
///
/// `<br>` always, because it is the only break available inside a table cell. A `|` only in a
/// table row, because a pipe in ordinary prose is a pipe.
fn split_segments(line: &str) -> Vec<Segment<'_>> {
    let cells = line.starts_with('|');
    let bytes = line.as_bytes();
    let mut segments = Vec::new();
    let mut start = 0;
    let mut cursor = 0;
    while cursor < line.len() {
        let boundary = if cells && bytes[cursor] == b'|' {
            Some(1)
        } else {
            line_break_at(line, cursor)
        };
        if let Some(length) = boundary {
            segments.push(Segment {
                text: &line[start..cursor],
                bounded: true,
            });
            cursor += length;
            start = cursor;
        } else {
            cursor += 1;
        }
    }
    segments.push(Segment {
        text: &line[start..],
        bounded: false,
    });
    segments
}

/// The length of a `<br>` tag at `at`, in any of the spellings a document uses.
fn line_break_at(line: &str, at: usize) -> Option<usize> {
    let bytes = line.as_bytes();
    if bytes.get(at) != Some(&b'<') {
        return None;
    }
    let mut index = at + 1;
    for expected in *b"br" {
        if bytes.get(index)?.to_ascii_lowercase() != expected {
            return None;
        }
        index += 1;
    }
    while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
        index += 1;
    }
    if bytes.get(index) == Some(&b'/') {
        index += 1;
        while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
            index += 1;
        }
    }
    if bytes.get(index) == Some(&b'>') {
        Some(index + 1 - at)
    } else {
        None
    }
}

/// Where an annotation's span ends inside its segment: at the closing backtick when the keyword
/// opened an inline-code span, and at the segment's end otherwise.
fn span_limit(text: &str, start: usize, after_keyword: usize) -> usize {
    if !text[..start].ends_with('`') || after_keyword > text.len() {
        return text.len();
    }
    text[after_keyword..]
        .find('`')
        .map_or(text.len(), |at| after_keyword + at)
}

/// Reads the date and separator that follow the keyword.
fn read_head(rest: &str) -> Option<Head<'_>> {
    let bytes = rest.as_bytes();
    let mut index = 0;
    while matches!(bytes.get(index), Some(b' ' | b'\t')) {
        index += 1;
    }
    let date = rest.get(index..index + DATE_LEN)?;
    if !is_iso_shaped(date) {
        return None;
    }
    index += DATE_LEN;
    while matches!(bytes.get(index), Some(b' ' | b'\t')) {
        index += 1;
    }
    let em_dash = rest[index..].starts_with(SEPARATOR);
    Some(Head {
        date,
        body_start: if em_dash {
            index + SEPARATOR.len_utf8()
        } else {
            index
        },
        em_dash,
    })
}

/// Whether a ten-character slice is shaped like `YYYY-MM-DD`. Says nothing about whether the
/// calendar has that day: [`CivilDate::parse`] decides that, and its refusal is a rejection with a
/// reason rather than a silent skip.
fn is_iso_shaped(date: &str) -> bool {
    let bytes = date.as_bytes();
    bytes.len() == DATE_LEN
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..].iter().all(u8::is_ascii_digit)
}

/// Where a body ends inside its segment: at the next annotation keyword, or at `limit`.
fn body_stop(text: &str, from: usize, limit: usize) -> usize {
    let tail = &text[from..limit];
    let next = [KEYWORD, OTHER_KEYWORD]
        .iter()
        .filter_map(|keyword| tail.find(keyword))
        .min();
    next.map_or(limit, |at| from + at)
}

/// Finds the last horizon token in a body, as `(start, end, horizon)`.
///
/// Strict on the left edge — `( horizon: 5d)` is not a token — and tolerant of case and of spaces
/// *inside* it. Both halves are deliberate. An implementation that rejects `(Horizon: 7D)` reports
/// a correctly annotated claim as undated prose; an implementation that loosens the left edge is
/// one step from accepting `(horizon: 5d — but see below)`, which is the failure the token's
/// strictness exists to prevent.
fn find_token(body: &str) -> Option<(usize, usize, Horizon)> {
    let mut found = None;
    for (open, _) in body.match_indices('(') {
        let rest = &body[open + 1..];
        if !rest
            .get(..TOKEN_KEYWORD.len())
            .is_some_and(|word| word.eq_ignore_ascii_case(TOKEN_KEYWORD))
        {
            continue;
        }
        let Some(value) = rest[TOKEN_KEYWORD.len()..].trim_start().strip_prefix(':') else {
            continue;
        };
        let Some(close) = value.find(')') else {
            continue;
        };
        let Ok(horizon) = Horizon::parse(&value[..close]) else {
            continue;
        };
        found = Some((open, body.len() - value.len() + close + 1, horizon));
    }
    found
}

/// Collapses every run of whitespace to one space, so a claim wrapped over three lines compares
/// equal to the same claim written on one.
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::{
        horizon_growth, raw_occurrences, scan, scan_at, ClaimRejectionReason, ClaimState,
        HorizonGrowth, DEFAULT_HORIZON_DAYS,
    };
    use aep_domain::time::{CivilDate, Horizon, ObservedAt};

    /// The corpus's reference date, so a test reads the same way in 2027.
    fn reference() -> CivilDate {
        CivilDate::parse("2026-09-01").expect("2026-09-01 is a date")
    }

    #[test]
    fn a_hyphen_separator_is_not_an_annotation_and_is_not_raw_counted() {
        let text = "Verify: 2026-08-30 - hyphen instead of em-dash. (horizon: 7d)\n";

        let result = scan(text);

        assert!(
            result.records.is_empty(),
            "a hyphen must produce no record, found {:?}",
            result.records
        );
        assert_eq!(
            result.raw_occurrences, 0,
            "a hyphen must not be raw-counted either, or the corpus count inflates and a real \
             malformed line hides in the noise"
        );
        assert_eq!(result.divergence(), 0, "so it is not a divergence either");
        assert_eq!(
            result.rejections.first().map(|rejection| rejection.reason),
            Some(ClaimRejectionReason::SeparatorNotEmDash),
            "and it is refused with a reason rather than dropped"
        );
    }

    #[test]
    fn a_space_after_the_opening_paren_leaves_a_record_carrying_the_stated_default() {
        let text = "Verify: 2026-08-30 — the pallet-fork latency is under 50ms. ( horizon: 5d)\n";

        let result = scan(text);

        let record = result.records.first().expect("still an annotation");
        assert!(
            record.malformed,
            "the token did not parse, so the flag is set"
        );
        assert_eq!(
            record.horizon.as_days(),
            DEFAULT_HORIZON_DAYS,
            "and the stated default applies rather than the 5d nobody managed to write"
        );
        assert!(
            record.claim.ends_with("( horizon: 5d)"),
            "the near-miss token stays in the claim: {}",
            record.claim
        );
    }

    #[test]
    fn prose_inside_the_token_does_not_parse() {
        let text =
            "Verify: 2026-08-20 — the grommet queue is drained. (horizon: 5d — superseded)\n";

        let record = scan(text).records.pop().expect("still an annotation");

        assert!(record.malformed, "a sentence is not a token");
        assert_eq!(
            record.horizon.as_days(),
            DEFAULT_HORIZON_DAYS,
            "so the 5d inside the prose is not the horizon"
        );
    }

    #[test]
    fn a_fourteen_day_horizon_written_by_hand_is_not_malformed() {
        let chosen = scan("Verify: 2026-08-28 — the index rebuild finished. (horizon: 14d)\n");
        let defaulted = scan("Verify: 2026-08-28 — the index rebuild finished.\n");

        let chosen = chosen.records.first().expect("a record");
        let defaulted = defaulted.records.first().expect("a record");
        assert_eq!(
            chosen.horizon, defaulted.horizon,
            "the two are indistinguishable by value, which is why the flag is carried"
        );
        assert!(
            !chosen.malformed,
            "14d is a legitimate choice, not a defect"
        );
        assert!(defaulted.malformed, "and the absent token still is one");
    }

    #[test]
    fn a_token_in_capitals_or_with_inner_spaces_still_parses() {
        let capitals = scan("Verify: 2026-08-30 — the queue depth is under 100. (Horizon: 7D)\n");
        let spaced =
            scan("Verify: 2026-08-30 — the health endpoint returns 200. (horizon:  5 d )\n");

        let capitals = capitals.records.first().expect("a record");
        let spaced = spaced.records.first().expect("a record");
        assert!(
            !capitals.malformed && capitals.horizon.as_days() == 7,
            "case-sensitivity would report a correctly annotated claim as undated prose"
        );
        assert!(
            !spaced.malformed && spaced.horizon.as_days() == 5,
            "and so would demanding exactly one space around the number"
        );
    }

    #[test]
    fn an_observation_exactly_its_horizon_old_is_not_expired() {
        let result = scan("Verify: 2026-08-25 — the canary is healthy. (horizon: 7d)\n");

        let state = result
            .records
            .first()
            .expect("a record")
            .state(reference(), 2);

        assert_eq!(
            state,
            ClaimState::Expiring { remaining_days: 0 },
            "age 7 against horizon 7 is the last covered day; expiring here fires the gate a day \
             early on every record in a corpus"
        );
    }

    #[test]
    fn one_day_past_the_horizon_is_expired_by_one_day() {
        let result = scan("Verify: 2026-08-24 — the rebuild completed. (horizon: 7d)\n");

        let state = result
            .records
            .first()
            .expect("a record")
            .state(reference(), 2);

        assert_eq!(state, ClaimState::Expired { overrun_days: 1 });
    }

    #[test]
    fn an_observation_dated_after_the_reference_date_is_refused_rather_than_recorded() {
        let text = "Verify: 2026-09-08 — the migration will have been applied. (horizon: 7d)\n";

        let result = scan_at(text, reference());

        assert!(
            result.records.is_empty(),
            "a scheduled check is not an observation, and a negative age would report it as the \
             freshest record in the corpus"
        );
        assert_eq!(
            result.rejections.first().map(|rejection| rejection.reason),
            Some(ClaimRejectionReason::FutureObservation)
        );
        assert_eq!(
            result.raw_occurrences, 1,
            "it is still annotation-shaped, so the raw count sees it"
        );
        assert_eq!(
            result.divergence(),
            1,
            "and the refusal is reported as a divergence rather than dropped silently"
        );
    }

    #[test]
    fn a_date_the_calendar_does_not_have_is_refused_with_a_reason() {
        let result =
            scan("Verify: 2026-02-30 — the leap year does not stretch that far. (horizon: 7d)\n");

        assert!(result.records.is_empty());
        assert_eq!(
            result.rejections.first().map(|rejection| rejection.reason),
            Some(ClaimRejectionReason::ImpossibleDate)
        );
    }

    #[test]
    fn two_adjacent_annotations_are_two_records() {
        let text = concat!(
            "Verify: 2026-08-30 — the deployment succeeded. (horizon: 7d)\n",
            "Verify: 2026-08-24 — the previous revision is still retained. (horizon: 3d)\n",
        );

        let result = scan(text);

        assert_eq!(
            result.records.len(),
            2,
            "absorbing the second yields one record with a stale date and the wrong horizon"
        );
        assert_eq!(result.records[1].horizon.as_days(), 3);
        assert!(
            !result.records[0].claim.contains("retained"),
            "the first body must stop at the second keyword: {}",
            result.records[0].claim
        );
    }

    #[test]
    fn a_body_stops_at_a_bare_quote_line() {
        let text = concat!(
            "> Verify: 2026-08-29 — the rework is not yet enabled. (horizon: 3d)\n",
            "> A wrapped sentence that belongs to it.\n",
            ">\n",
            "> This sentence must not be absorbed.\n",
        );

        let record = scan(text).records.pop().expect("a record in a quote block");

        assert!(
            record.claim.contains("wrapped sentence"),
            "the continuation belongs to the body: {}",
            record.claim
        );
        assert!(
            !record.claim.contains("must not be absorbed"),
            "the bare `>` line is a stop: {}",
            record.claim
        );
    }

    #[test]
    fn an_annotation_ending_a_table_cell_with_no_break_is_found() {
        let text = "| WF-401 | open | A long note about the regression. Verify: 2026-08-30 — read \
                    from the running image. (horizon: 5d) |\n";

        let result = scan(text);

        let record = result.records.first().expect("the highest-volume gap");
        assert_eq!(record.horizon.as_days(), 5);
        assert_eq!(
            record.claim, "read from the running image.",
            "the cell's earlier prose is not part of the claim, and the closing pipe is not either"
        );
        assert_eq!(result.divergence(), 0);
    }

    #[test]
    fn the_second_of_two_consecutive_break_rows_keeps_its_own_date_and_horizon() {
        let text = concat!(
            "| mainspring | 3.0.1 | notes<br>Verify: 2026-08-30 — pin read. (horizon: 7d) |\n",
            "| balance-wheel | 1.9.4 | notes<br>Verify: 2026-08-29 — pin read. (horizon: 2d) |\n",
        );

        let result = scan(text);

        assert_eq!(
            result.records.len(),
            2,
            "swallowing the second loses a shorter-horizon, older claim behind a fresher neighbour"
        );
        assert_eq!(
            result.records[1].horizon.as_days(),
            2,
            "its horizon survives"
        );
        assert_eq!(
            result.records[1].date.to_string(),
            "2026-08-29",
            "and so does its date"
        );
        assert_eq!(result.divergence(), 0);
    }

    #[test]
    fn an_annotation_inside_inline_code_is_found_without_its_backticks() {
        let text = "`Verify: 2026-08-30 — the rework is enabled in atlas. (horizon: 3d)`\n";

        let record = scan(text).records.pop().expect("a record");

        assert_eq!(record.claim, "the rework is enabled in atlas.");
        assert_eq!(record.horizon.as_days(), 3);
    }

    #[test]
    fn a_malformed_record_renders_its_source_rather_than_its_default() {
        let text = "Verify: 2026-08-25 — the rollout reached all three zones.\n";

        let record = scan(text).records.pop().expect("a record");

        assert_eq!(
            record.render(),
            text.trim(),
            "rendering the 14d default into the document would turn a carried flag into an \
             inferred one"
        );
        assert!(
            !record.render().contains("horizon:"),
            "and would state a horizon nobody chose"
        );
    }

    #[test]
    fn a_well_formed_record_renders_the_canonical_line() {
        let text = "  Verify: 2026-08-30 — the queue depth is under 100. (Horizon: 7D)\n";

        let record = scan(text).records.pop().expect("a record");

        assert_eq!(
            record.render(),
            "Verify: 2026-08-30 — the queue depth is under 100. (horizon: 7d)"
        );
    }

    #[test]
    fn horizon_growth_ignores_a_pair_whose_observation_date_moved() {
        let claim = "the grommet index row count matches the primary store.";
        let before = scan(&format!("Verify: 2026-08-04 — {claim} (horizon: 7d)\n"));
        let grown = scan(&format!("Verify: 2026-08-04 — {claim} (horizon: 60d)\n"));
        let rechecked = scan(&format!("Verify: 2026-08-30 — {claim} (horizon: 7d)\n"));

        let flagged = horizon_growth(&before, &grown);
        let ignored = horizon_growth(&before, &rechecked);

        assert_eq!(
            flagged,
            vec![HorizonGrowth::Grew {
                claim: claim.to_string(),
                date: CivilDate::parse("2026-08-04").expect("a date"),
                observed_at: ObservedAt::new(
                    CivilDate::parse("2026-08-04")
                        .expect("a date")
                        .to_timestamp()
                ),
                before: Horizon::days(7).expect("a horizon"),
                after: Horizon::days(60).expect("a horizon"),
            }],
            "the horizon grew and the date stood still"
        );
        assert!(
            ignored.is_empty(),
            "re-checking is the correct refresh and must not be flagged: {ignored:?}"
        );
    }

    #[test]
    fn a_claim_stated_twice_at_one_date_is_reported_ambiguous_rather_than_paired() {
        let claim = "the grommet index row count matches the primary store.";
        let twice = scan(&format!(
            "Verify: 2026-08-04 — {claim} (horizon: 7d)\n\nVerify: 2026-08-04 — {claim} (horizon: 30d)\n"
        ));
        let once = scan(&format!("Verify: 2026-08-04 — {claim} (horizon: 60d)\n"));

        assert_eq!(twice.records.len(), 2, "the fixture states it twice");
        let findings = horizon_growth(&twice, &once);

        assert_eq!(findings.len(), 1);
        assert!(
            !findings[0].is_growth(),
            "which record grew into which is not decidable, so the pairing is refused: {:?}",
            findings[0]
        );
        assert_eq!(findings[0].claim(), claim);
    }

    #[test]
    fn a_body_stops_at_every_line_a_reader_would_call_a_new_one() {
        let cases = [
            (
                "another annotation",
                "Verify: 2026-08-24 — a second reading.",
            ),
            (
                "a scheduled action",
                "Due: 2026-09-10 — apply the migration.",
            ),
            ("a heading", "### A heading that must not be absorbed"),
            (
                "a list bullet",
                "- an unrelated bullet that belongs to nothing",
            ),
            ("a blank line", ""),
            ("a table row", "| a | new | row |"),
        ];

        for (what, stop) in cases {
            let text = format!(
                "Verify: 2026-08-30 — the migration has not been applied.\n{stop}\ntrailing prose\n"
            );

            let record = scan(&text).records.remove(0);

            assert!(
                !record.claim.contains("trailing prose"),
                "{what} must stop the body, and did not: {}",
                record.claim
            );
            assert_eq!(
                record.claim, "the migration has not been applied.",
                "{what} must not be absorbed either"
            );
        }
    }

    #[test]
    fn the_raw_count_sees_an_annotation_the_parser_is_given_no_chance_at() {
        // The counter shares no code with the parser: it is the scan's independent denominator.
        let text = "Verify: 2026-08-30 — a claim. (horizon: 7d)\n";

        assert_eq!(raw_occurrences(text), 1);
        assert_eq!(raw_occurrences("Verify: 2026-08-30 - hyphen.\n"), 0);
        assert_eq!(raw_occurrences("a mention of `Verify:` in prose\n"), 0);
    }
}
