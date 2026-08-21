//! Timestamps, calendar dates, and how long an observation is worth something.
//!
//! The domain crate is deliberately clock-free: a [`Timestamp`] can be constructed from an
//! epoch value but never read from the system clock here. Wall-clock access belongs to the
//! engine, behind a `Clock` it can swap for a fixed one in tests, which is what makes an
//! execution replayable.
//!
//! Four types, and the difference between the last three is the design:
//!
//! | type | what it is | who supplies it |
//! |---|---|---|
//! | [`Timestamp`] | an instant, in epoch milliseconds | whoever holds a clock |
//! | [`CivilDate`] | a day, as a person writes one in a document | the document |
//! | [`Horizon`] | how long an observation is worth something | the **requirement** |
//! | [`ObservedAt`] | when somebody looked | the **caller** submitting evidence |
//!
//! A record's `produced_at` says when it entered the log; its [`ObservedAt`] says when the
//! observation happened. Collapsing the two is the defect
//! `docs/design/evidence-horizons-design-v0.1.md` exists to remove: with one field, the only
//! honest thing a harness can do with an old observation is lie about it.

use std::fmt;

use crate::error::ParseError;

/// Milliseconds since the Unix epoch, UTC.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(transparent)]
pub struct Timestamp(u64);

impl Timestamp {
    /// The epoch itself, useful as a deterministic default in tests.
    pub const EPOCH: Self = Self(0);

    /// Builds a timestamp from milliseconds since the Unix epoch.
    pub const fn from_epoch_millis(millis: u64) -> Self {
        Self(millis)
    }

    /// Milliseconds since the Unix epoch.
    pub const fn epoch_millis(self) -> u64 {
        self.0
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}ms", self.0)
    }
}

/// A calendar date, with no time of day and no zone.
///
/// Evidence horizons are written by people, in documents, as `2026-08-30` — a day, not an instant.
/// Converting one to a [`Timestamp`] needs civil-calendar arithmetic and nothing else: no clock, no
/// zone database, no dependency. The conversion is Howard Hinnant's `days_from_civil`, which is
/// exact for every proleptic Gregorian date and is pure integer arithmetic, so this type keeps the
/// crate's clock-free property (invariant 8) while still letting a document say a date.
///
/// Midnight UTC is the instant a date maps to. That choice is stated rather than assumed because it
/// decides a boundary: an observation on `2026-08-25` with a seven-day horizon, read on
/// `2026-09-01`, is exactly seven days old and therefore **still covered** — see
/// [`Horizon::covers`].
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, schemars::JsonSchema,
)]
pub struct CivilDate {
    /// The proleptic Gregorian year.
    year: i32,
    /// The month, 1 to 12.
    month: u32,
    /// The day of the month, 1 to the month's length.
    day: u32,
}

/// Milliseconds in one day.
const MILLIS_PER_DAY: u64 = 86_400_000;

impl CivilDate {
    /// Builds a date, refusing one the calendar does not have.
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError::Reference`] when the month is outside 1–12 or the day is outside the
    /// month's length, leap years included.
    pub fn new(year: i32, month: u32, day: u32) -> Result<Self, ParseError> {
        let written = format!("{year:04}-{month:02}-{day:02}");
        if !(1..=12).contains(&month) {
            return Err(ParseError::reference(
                "date",
                &written,
                "the month is outside 1 to 12",
            ));
        }
        let length = Self::days_in_month(year, month);
        if day < 1 || day > length {
            return Err(ParseError::reference(
                "date",
                &written,
                format!("{year:04}-{month:02} has {length} days"),
            ));
        }
        Ok(Self { year, month, day })
    }

    /// Parses the ISO form `YYYY-MM-DD`, which is the only form documents use.
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError::Reference`] when the shape is not three `-`-separated numbers, or
    /// when the date they name does not exist.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        let mut parts = value.split('-');
        let (Some(year), Some(month), Some(day), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(ParseError::reference(
                "date",
                value,
                "expected the form YYYY-MM-DD",
            ));
        };
        if year.len() != 4 || month.len() != 2 || day.len() != 2 {
            return Err(ParseError::reference(
                "date",
                value,
                "expected the form YYYY-MM-DD, zero-padded",
            ));
        }
        let number = |part: &str, what: &'static str| -> Result<i64, ParseError> {
            part.parse::<i64>().map_err(|_| {
                ParseError::reference("date", value, format!("the {what} is not a number"))
            })
        };
        let year = number(year, "year")?;
        let month = number(month, "month")?;
        let day = number(day, "day")?;
        let (Ok(year), Ok(month), Ok(day)) = (
            i32::try_from(year),
            u32::try_from(month),
            u32::try_from(day),
        ) else {
            return Err(ParseError::reference(
                "date",
                value,
                "the date is out of range",
            ));
        };
        Self::new(year, month, day)
    }

    /// The year.
    pub fn year(self) -> i32 {
        self.year
    }

    /// The month, 1 to 12.
    pub fn month(self) -> u32 {
        self.month
    }

    /// The day of the month.
    pub fn day(self) -> u32 {
        self.day
    }

    /// Days since 1970-01-01, negative before it.
    ///
    /// Howard Hinnant's `days_from_civil`: shift the year so March starts the era, count whole
    /// eras of 400 years, then days within the era. Exact, branch-light and dependency-free.
    pub fn days_from_epoch(self) -> i64 {
        let year = i64::from(self.year) - i64::from(self.month <= 2);
        let era = if year >= 0 { year } else { year - 399 } / 400;
        let year_of_era = year - era * 400;
        let month = i64::from(self.month);
        let day_of_year =
            (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + i64::from(self.day) - 1;
        let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
        era * 146_097 + day_of_era - 719_468
    }

    /// Midnight UTC on this date, as a [`Timestamp`].
    ///
    /// Dates before the epoch clamp to [`Timestamp::EPOCH`]: a [`Timestamp`] is unsigned, and an
    /// evidence horizon over a 1969 observation is not a case this model owes an answer to.
    pub fn to_timestamp(self) -> Timestamp {
        let days = self.days_from_epoch().max(0);
        Timestamp::from_epoch_millis(
            u64::try_from(days)
                .unwrap_or(0)
                .saturating_mul(MILLIS_PER_DAY),
        )
    }

    /// The date an instant falls on, in UTC.
    ///
    /// The inverse of [`Self::to_timestamp`], and the reason a refusal can name a day rather than
    /// an epoch millisecond count: a horizon is a number of days, so the reader of *"the last
    /// observation was on 2026-08-25"* has everything the arithmetic used.
    pub fn from_timestamp(at: Timestamp) -> Self {
        let days = i64::try_from(at.epoch_millis() / MILLIS_PER_DAY).unwrap_or(i64::MAX);
        Self::from_days_from_epoch(days)
    }

    /// The date `days` days after 1970-01-01; Howard Hinnant's `civil_from_days`.
    fn from_days_from_epoch(days: i64) -> Self {
        let shifted = days + 719_468;
        let era = if shifted >= 0 {
            shifted
        } else {
            shifted - 146_096
        } / 146_097;
        let day_of_era = shifted - era * 146_097;
        let year_of_era =
            (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let year = year_of_era + era * 400;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let shifted_month = (5 * day_of_year + 2) / 153;
        let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
        let month = if shifted_month < 10 {
            shifted_month + 3
        } else {
            shifted_month - 9
        };
        Self {
            year: i32::try_from(year + i64::from(month <= 2)).unwrap_or(i32::MAX),
            month: u32::try_from(month).unwrap_or(1),
            day: u32::try_from(day).unwrap_or(1),
        }
    }

    /// How many days this date is after `other`, negative when it is before.
    pub fn days_since(self, other: Self) -> i64 {
        self.days_from_epoch() - other.days_from_epoch()
    }

    /// The length of a month, leap years included.
    fn days_in_month(year: i32, month: u32) -> u32 {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if Self::is_leap(year) => 29,
            2 => 28,
            _ => 0,
        }
    }

    /// Whether `year` is a leap year in the proleptic Gregorian calendar.
    fn is_leap(year: i32) -> bool {
        (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
    }
}

impl fmt::Display for CivilDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl std::str::FromStr for CivilDate {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl<'de> serde::Deserialize<'de> for CivilDate {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let written = String::deserialize(deserializer)?;
        Self::parse(&written).map_err(serde::de::Error::custom)
    }
}

/// How long an observation is worth something, in whole days.
///
/// A horizon is a **volatility guess**, never a guarantee: a claim written with a seven-day horizon
/// can be false on day five, and no clock can detect it. What a horizon buys is the other failure
/// mode — an observation nobody has repeated stops reading as a fact, and the transition it used to
/// permit is refused with the reason *nobody knows*.
///
/// Days, not milliseconds, because that is the unit the convention is written in
/// (`(horizon: 7d)`) and a horizon expressed more precisely than the observation would be false
/// precision.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, schemars::JsonSchema,
)]
#[serde(transparent)]
pub struct Horizon {
    /// The number of whole days.
    days: u32,
}

impl Horizon {
    /// The longest horizon accepted: ten years.
    ///
    /// A bound exists so that a typo — `(horizon: 70000d)` for `7000d` — is refused rather than
    /// stored as a horizon nothing will ever outlive, which is a horizon that has been switched off
    /// while still looking like one.
    pub const MAX_DAYS: u32 = 3650;

    /// A horizon of `days` days.
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError::Reference`] for zero — an observation that is stale the moment it is
    /// made is a refusal, not a horizon — or for more than [`Self::MAX_DAYS`].
    pub fn days(days: u32) -> Result<Self, ParseError> {
        if days == 0 {
            return Err(ParseError::reference(
                "horizon",
                &format!("{days}d"),
                "a horizon of zero days admits no observation at all",
            ));
        }
        if days > Self::MAX_DAYS {
            return Err(ParseError::reference(
                "horizon",
                &format!("{days}d"),
                format!("the longest horizon is {}d", Self::MAX_DAYS),
            ));
        }
        Ok(Self { days })
    }

    /// Parses the written form: `7d`, `7 d`, `7D` or a bare `7`.
    ///
    /// Deliberately tolerant of case and of spaces *inside* the value, and deliberately strict
    /// about everything else. `examples/evidence-horizons-corpus/corpus/01-forms.md` establishes
    /// both halves: an implementation that rejects `(Horizon: 7D)` reports a correctly annotated
    /// claim as undated prose, and an implementation that accepts a phrase where a token belongs
    /// ends up accepting `(horizon: 5d — but see below)`, which is the failure the token's
    /// strictness exists to prevent.
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError::Reference`] when the value is not a number optionally followed by
    /// `d`, or when the number is outside the accepted range.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        let trimmed = value.trim();
        let digits = match trimmed.strip_suffix(['d', 'D']) {
            Some(rest) => rest.trim_end(),
            None => trimmed,
        };
        if digits.is_empty() || !digits.chars().all(|character| character.is_ascii_digit()) {
            return Err(ParseError::reference(
                "horizon",
                value,
                "expected a number of days, such as `7d`",
            ));
        }
        let days = digits.parse::<u32>().map_err(|_| {
            ParseError::reference("horizon", value, "the number of days is out of range")
        })?;
        Self::days(days)
    }

    /// The horizon in whole days.
    pub fn as_days(self) -> u32 {
        self.days
    }

    /// The horizon in milliseconds.
    pub fn as_millis(self) -> u64 {
        u64::from(self.days) * MILLIS_PER_DAY
    }

    /// When an observation made at `observed_at` stops being covered.
    pub fn expires_at(self, observed_at: Timestamp) -> Timestamp {
        Timestamp::from_epoch_millis(observed_at.epoch_millis().saturating_add(self.as_millis()))
    }

    /// Whether an observation made at `observed_at` still stands at `now`.
    ///
    /// `age == horizon` is **covered**, not expired. The corpus states the cost of the other
    /// choice: *"An off-by-one there fires the gate a day early on every record in a corpus, which
    /// is how a gate gets muted and then deleted."*
    pub fn covers(self, observed_at: Timestamp, now: Timestamp) -> bool {
        now.epoch_millis()
            .saturating_sub(observed_at.epoch_millis())
            <= self.as_millis()
    }
}

impl fmt::Display for Horizon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}d", self.days)
    }
}

impl std::str::FromStr for Horizon {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl<'de> serde::Deserialize<'de> for Horizon {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let written = crate::node::Node::deserialize(deserializer)?;
        match &written {
            crate::node::Node::Text(text) => Self::parse(text).map_err(serde::de::Error::custom),
            crate::node::Node::Number(number) => {
                Self::parse(&number.to_string()).map_err(serde::de::Error::custom)
            }
            other => Err(serde::de::Error::custom(format!(
                "expected a horizon such as `7d`, found {}",
                other.type_name()
            ))),
        }
    }
}

/// When somebody looked.
///
/// The second of an evidence record's two times, and the one the protocol reasons about. The
/// engine's [`EvidenceEnvelope::produced_at`](crate::evidence::EvidenceEnvelope::produced_at) says
/// when the record entered the log; this says when the observation behind it was made, and the
/// caller supplies it because the caller is the only party that knows.
///
/// # Why a type rather than a second `Timestamp`
///
/// Two adjacent parameters of one type is a swap waiting to happen, and a swapped pair here is
/// silent — both are plausible epoch values, and the wrong one decays on the wrong clock. This
/// makes the swap a compile error, and gives the future-date comparison and the age computation one
/// home rather than one per call site.
///
/// # A future value is not a fresh record
///
/// A planned re-check is a different object from a decaying observation. Under a one-field
/// convention, a scheduled-but-never-performed check reads as the *freshest* record there is —
/// a negative age inflates the remaining horizon — so the model can no longer answer *has anyone
/// ever looked at this?*. [`Self::is_after`] is the one comparison that makes the conflation
/// unwritable; the engine calls it before a record is stored.
/// # How it is written
///
/// Serialised as epoch milliseconds, because that is what it is. **Accepted** as either — an
/// instant, or the calendar date a person writes in a document:
///
/// ```yaml
/// observed_at: 2026-08-30        # midnight UTC on that day
/// observed_at: 1788134400000     # the same instant, exactly
/// ```
///
/// The alias is deliberate and follows the convention this repository already keeps for
/// `unit_tests.failed` beside `tests.unit.failed`: the canonical form is what the engine emits, the
/// second spelling is only accepted on input. A hand-written evidence document whose observation
/// time is a thirteen-digit number is a document nobody checks by reading.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, schemars::JsonSchema,
)]
#[serde(transparent)]
pub struct ObservedAt(Timestamp);

impl ObservedAt {
    /// An observation made at `at`.
    pub const fn new(at: Timestamp) -> Self {
        Self(at)
    }

    /// The instant itself.
    pub const fn timestamp(self) -> Timestamp {
        self.0
    }

    /// Whether this observation is claimed to have happened after `now`.
    ///
    /// `true` is a refusal, never a fresh record.
    pub const fn is_after(self, now: Timestamp) -> bool {
        self.0.epoch_millis() > now.epoch_millis()
    }

    /// How old the observation is at `now`, in milliseconds; zero when it is not yet past.
    pub const fn age_millis(self, now: Timestamp) -> u64 {
        now.epoch_millis().saturating_sub(self.0.epoch_millis())
    }

    /// How old the observation is at `now`, in whole days.
    pub const fn age_days(self, now: Timestamp) -> u64 {
        self.age_millis(now) / MILLIS_PER_DAY
    }
}

impl<'de> serde::Deserialize<'de> for ObservedAt {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let written = crate::node::Node::deserialize(deserializer)?;
        match &written {
            crate::node::Node::Text(text) => Ok(Self(
                CivilDate::parse(text)
                    .map_err(serde::de::Error::custom)?
                    .to_timestamp(),
            )),
            crate::node::Node::Number(number) => {
                let millis = number.to_string();
                millis
                    .parse::<u64>()
                    .map(|millis| Self(Timestamp::from_epoch_millis(millis)))
                    .map_err(|_| {
                        serde::de::Error::custom(format!(
                            "an observation time is a date such as 2026-08-30 or epoch \
                             milliseconds, found {millis}"
                        ))
                    })
            }
            other => Err(serde::de::Error::custom(format!(
                "an observation time is a date such as 2026-08-30 or epoch milliseconds, found {}",
                other.type_name()
            ))),
        }
    }
}

impl From<Timestamp> for ObservedAt {
    fn from(at: Timestamp) -> Self {
        Self(at)
    }
}

impl fmt::Display for ObservedAt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_date_maps_to_midnight_utc_on_that_day() {
        let date = CivilDate::parse("1970-01-02").expect("a valid date");
        assert_eq!(date.to_timestamp().epoch_millis(), MILLIS_PER_DAY);
        assert_eq!(
            CivilDate::parse("1970-01-01")
                .expect("epoch")
                .to_timestamp(),
            Timestamp::EPOCH
        );
    }

    #[test]
    fn every_date_in_the_corpus_survives_the_round_trip_through_an_instant() {
        for written in [
            "1970-01-01",
            "2026-06-20",
            "2026-08-04",
            "2026-08-25",
            "2026-08-31",
            "2026-09-01",
            "2024-02-29",
            "2000-02-29",
            "2100-03-01",
            "2027-03-31",
        ] {
            let date = CivilDate::parse(written).expect("a valid date");
            assert_eq!(
                CivilDate::from_timestamp(date.to_timestamp()),
                date,
                "{written} must survive the round trip"
            );
            assert_eq!(date.to_string(), written);
        }
    }

    #[test]
    fn the_corpus_reference_date_is_seven_days_after_its_boundary_observation() {
        // `corpus/04-classification.md`: age 7, horizon 7, and the record must classify ok.
        let observed = CivilDate::parse("2026-08-25").expect("a valid date");
        let reference = CivilDate::parse("2026-09-01").expect("a valid date");
        assert_eq!(reference.days_since(observed), 7);
    }

    #[test]
    fn a_date_the_calendar_does_not_have_is_refused_by_name() {
        let error = CivilDate::parse("2026-02-30").expect_err("February has no thirtieth");
        assert!(error.to_string().contains("28 days"), "{error}");
        let leap = CivilDate::parse("2024-02-29").expect("2024 is a leap year");
        assert_eq!(leap.day(), 29);
        let century = CivilDate::parse("1900-02-29").expect_err("1900 is not a leap year");
        assert!(century.to_string().contains("28 days"), "{century}");
        assert!(
            CivilDate::parse("2000-02-29").is_ok(),
            "2000 is a leap year"
        );
    }

    #[test]
    fn a_date_without_zero_padding_is_refused_rather_than_guessed() {
        let error = CivilDate::parse("2026-8-30").expect_err("the form is fixed");
        assert!(error.to_string().contains("zero-padded"), "{error}");
    }

    #[test]
    fn the_horizon_token_is_read_in_every_spelling_the_corpus_uses() {
        for written in ["7d", "7D", " 7 d ", "7"] {
            assert_eq!(
                Horizon::parse(written).expect("a valid horizon").as_days(),
                7,
                "{written}"
            );
        }
        assert_eq!(Horizon::days(7).expect("a valid horizon").to_string(), "7d");
    }

    #[test]
    fn a_horizon_that_is_a_phrase_is_not_a_horizon() {
        // `corpus/02-malformed.md`: "(horizon: 5d — superseded by the re-read above)". Accepting
        // the prose is how the convention decays into undated text one annotation at a time.
        let error = Horizon::parse("5d — superseded by the re-read above").expect_err("a phrase");
        assert!(error.to_string().contains("number of days"), "{error}");
        assert!(
            Horizon::parse("").is_err(),
            "an empty token is not a horizon"
        );
        assert!(Horizon::days(0).is_err(), "zero admits no observation");
        assert!(
            Horizon::days(Horizon::MAX_DAYS + 1).is_err(),
            "the bound holds"
        );
    }

    #[test]
    fn an_observation_exactly_its_horizon_old_is_still_covered() {
        let observed = CivilDate::parse("2026-08-25")
            .expect("a date")
            .to_timestamp();
        let horizon = Horizon::days(7).expect("a horizon");
        let boundary = CivilDate::parse("2026-09-01")
            .expect("a date")
            .to_timestamp();
        assert!(
            horizon.covers(observed, boundary),
            "age == horizon is not expired"
        );
        let over = CivilDate::parse("2026-09-02")
            .expect("a date")
            .to_timestamp();
        assert!(!horizon.covers(observed, over), "one day over is expired");
        assert_eq!(horizon.expires_at(observed), boundary);
    }

    #[test]
    fn an_observation_time_is_written_as_a_date_or_as_an_instant_and_means_the_same_thing() {
        let by_date: ObservedAt = serde_yaml::from_str("2026-08-30").expect("a date parses");
        let by_instant: ObservedAt =
            serde_yaml::from_str(&by_date.timestamp().epoch_millis().to_string())
                .expect("an instant parses");
        assert_eq!(by_date, by_instant, "the two spellings are one value");
        assert_eq!(
            serde_yaml::to_string(&by_date).expect("serialises").trim(),
            by_date.timestamp().epoch_millis().to_string(),
            "the canonical form is the instant"
        );
        let refusal = serde_yaml::from_str::<ObservedAt>("last Tuesday")
            .expect_err("prose is not an observation time");
        assert!(refusal.to_string().contains("YYYY-MM-DD"), "{refusal}");
    }

    #[test]
    fn a_horizon_over_an_observation_that_is_not_at_midnight_is_exact_to_the_instant() {
        // The engine compares instants; a document scanner compares whole days. The two agree for
        // an observation at midnight and deliberately do not for one at noon, so the difference is
        // asserted here rather than discovered by a gate that fires half a day early.
        let noon = Timestamp::from_epoch_millis(
            CivilDate::parse("2026-08-25")
                .expect("a date")
                .to_timestamp()
                .epoch_millis()
                + MILLIS_PER_DAY / 2,
        );
        let horizon = Horizon::days(7).expect("a horizon");
        let midnight_on_the_first = CivilDate::parse("2026-09-01")
            .expect("a date")
            .to_timestamp();

        assert!(
            horizon.covers(noon, midnight_on_the_first),
            "six and a half days is inside seven"
        );
        assert!(
            horizon.covers(noon, horizon.expires_at(noon)),
            "the boundary instant itself is covered, exactly as the boundary day is"
        );
        assert!(
            !horizon.covers(
                noon,
                Timestamp::from_epoch_millis(horizon.expires_at(noon).epoch_millis() + 1)
            ),
            "and one millisecond past it is not"
        );
        assert_eq!(
            ObservedAt::new(noon).age_days(midnight_on_the_first),
            6,
            "whole days truncate, which is why a scanner over dates is a different computation"
        );
    }

    #[test]
    fn an_observation_in_the_future_is_recognised_rather_than_aged_backwards() {
        let now = Timestamp::from_epoch_millis(1_000);
        let ahead = ObservedAt::new(Timestamp::from_epoch_millis(2_000));
        assert!(
            ahead.is_after(now),
            "the comparison that makes the conflation unwritable"
        );
        assert_eq!(ahead.age_millis(now), 0, "an age is never negative");
        let behind = ObservedAt::new(Timestamp::EPOCH);
        assert!(!behind.is_after(now));
        assert_eq!(behind.age_millis(now), 1_000);
    }
}
