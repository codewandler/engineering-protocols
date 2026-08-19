//! What a conformance run produces.
//!
//! A report says which **property** failed, not which assertion fired. That distinction is the
//! difference between a suite someone can act on and one they have to read the source of: "a replay
//! applied the command twice" tells an implementer what to fix; "assertion failed at line 214" does
//! not.

use std::fmt;

/// One property a conforming backend must have.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Check {
    /// The property, stated as a sentence about the backend.
    pub name: String,
    /// Whether it holds.
    pub passed: bool,
    /// What was observed, when it does not hold — or a note worth keeping when it does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Check {
    /// A property that holds.
    pub fn passed(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed: true,
            detail: None,
        }
    }

    /// A property that does not hold, with what was observed instead.
    pub fn failed(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed: false,
            detail: Some(detail.into()),
        }
    }

    /// A property that holds or not, according to `condition`.
    pub fn expect(name: impl Into<String>, condition: bool, detail: impl Into<String>) -> Self {
        if condition {
            Self::passed(name)
        } else {
            Self::failed(name, detail)
        }
    }
}

impl fmt::Display for Check {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mark = if self.passed { '\u{2713}' } else { '\u{2717}' };
        write!(f, "{mark} {}", self.name)?;
        if let Some(detail) = &self.detail {
            if !self.passed {
                write!(f, " — {detail}")?;
            }
        }
        Ok(())
    }
}

/// The result of one suite.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SuiteReport {
    /// Which suite.
    pub suite: &'static str,
    /// What it checked.
    pub checks: Vec<Check>,
}

impl SuiteReport {
    /// An empty report for `suite`.
    pub fn new(suite: &'static str) -> Self {
        Self {
            suite,
            checks: Vec::new(),
        }
    }

    /// Records a check.
    pub fn record(&mut self, check: Check) {
        self.checks.push(check);
    }

    /// Records a property that holds or not.
    pub fn expect(&mut self, name: impl Into<String>, condition: bool, detail: impl Into<String>) {
        self.record(Check::expect(name, condition, detail));
    }

    /// Records that the backend failed in a way the suite could not continue past.
    ///
    /// Distinct from a failed property: the suite could not even ask the question, which usually
    /// means an earlier step did not work rather than that this one is wrong.
    pub fn aborted(&mut self, name: impl Into<String>, detail: impl Into<String>) {
        self.record(Check::failed(
            name,
            format!("could not be checked: {}", detail.into()),
        ));
    }

    /// `true` when every property holds.
    pub fn passed(&self) -> bool {
        self.checks.iter().all(|check| check.passed)
    }

    /// The properties that do not hold.
    pub fn failures(&self) -> impl Iterator<Item = &Check> {
        self.checks.iter().filter(|check| !check.passed)
    }

    /// How many properties were checked.
    pub fn len(&self) -> usize {
        self.checks.len()
    }

    /// `true` when the suite checked nothing, which is itself a failure of the suite.
    pub fn is_empty(&self) -> bool {
        self.checks.is_empty()
    }
}

impl fmt::Display for SuiteReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{} — {}/{} properties hold",
            self.suite,
            self.checks.iter().filter(|check| check.passed).count(),
            self.checks.len()
        )?;
        for check in &self.checks {
            writeln!(f, "  {check}")?;
        }
        Ok(())
    }
}

/// How much of the contract a backend claims to implement.
///
/// Levels exist so a backend can be useful before it is complete, and so what it claims is checkable
/// rather than described in a README.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    /// Identity, commands, idempotency, concurrency, query. Enough to store and retrieve work
    /// without losing it.
    Core,
    /// Core, plus audit, correlation, causation and provenance. Enough to reconstruct what happened.
    Audited,
    /// Everything. Enough to run an engineering protocol against.
    #[default]
    Full,
}

impl Level {
    /// Every level, weakest first.
    pub const ALL: &'static [Self] = &[Self::Core, Self::Audited, Self::Full];

    /// The level as written on a command line.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Audited => "audited",
            Self::Full => "full",
        }
    }

    /// Parses a level name.
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|level| level.as_str() == value)
    }

    /// `true` when this level includes everything `other` requires.
    pub fn includes(self, other: Self) -> bool {
        self >= other
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The result of a whole conformance run.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ConformanceReport {
    /// The level that was claimed.
    pub level: Level,
    /// What each suite found.
    pub suites: Vec<SuiteReport>,
}

impl ConformanceReport {
    /// `true` when every property in every suite holds.
    pub fn passed(&self) -> bool {
        self.suites.iter().all(SuiteReport::passed)
    }

    /// How many properties were checked in total.
    pub fn checks(&self) -> usize {
        self.suites.iter().map(SuiteReport::len).sum()
    }

    /// How many do not hold.
    pub fn failures(&self) -> usize {
        self.suites
            .iter()
            .map(|suite| suite.failures().count())
            .sum()
    }

    /// The suites that did not pass.
    pub fn failing_suites(&self) -> impl Iterator<Item = &SuiteReport> {
        self.suites.iter().filter(|suite| !suite.passed())
    }
}

impl fmt::Display for ConformanceReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for suite in &self.suites {
            write!(f, "{suite}")?;
        }
        if self.passed() {
            write!(
                f,
                "conformance {}: {} properties hold",
                self.level,
                self.checks()
            )
        } else {
            write!(
                f,
                "conformance {}: {} of {} properties do not hold",
                self.level,
                self.failures(),
                self.checks()
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_states_which_property_failed_not_which_assertion() {
        let mut suite = SuiteReport::new("idempotency");
        suite.expect("a replay returns the original result", true, "");
        suite.expect(
            "a replay applies the command once",
            false,
            "the entity advanced to revision 3 after replaying a command that produced revision 2",
        );

        assert!(!suite.passed());
        assert_eq!(suite.failures().count(), 1);
        let rendered = suite.to_string();
        assert!(rendered.contains("1/2 properties hold"), "{rendered}");
        assert!(rendered.contains("advanced to revision 3"), "{rendered}");
    }

    #[test]
    fn an_empty_suite_is_reported_as_empty_rather_than_passing() {
        let suite = SuiteReport::new("audit");
        assert!(suite.is_empty());
        assert!(
            suite.passed(),
            "vacuously, which is why the runner refuses an empty suite separately"
        );
    }

    #[test]
    fn levels_are_ordered_so_a_claim_can_be_checked() {
        assert!(Level::Full.includes(Level::Core));
        assert!(Level::Audited.includes(Level::Core));
        assert!(!Level::Core.includes(Level::Audited));
        assert_eq!(Level::parse("audited"), Some(Level::Audited));
        assert_eq!(Level::parse("thorough"), None);
    }

    #[test]
    fn a_run_counts_failures_across_suites() {
        let mut first = SuiteReport::new("identity");
        first.expect("identity is opaque", true, "");
        let mut second = SuiteReport::new("audit");
        second.expect(
            "a refusal is recorded",
            false,
            "no audit record was written",
        );

        let report = ConformanceReport {
            level: Level::Full,
            suites: vec![first, second],
        };
        assert!(!report.passed());
        assert_eq!(report.checks(), 2);
        assert_eq!(report.failures(), 1);
        assert_eq!(report.failing_suites().count(), 1);
        assert!(report.to_string().contains("1 of 2 properties do not hold"));
    }
}
