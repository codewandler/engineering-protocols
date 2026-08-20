//! What a run produces, and what a failure has to say for itself.
//!
//! Design §28 for the statuses, §29 for the diagnostics, §30 for the report. The shape follows
//! `aep_conformance::report`, which already solved the per-check result structure and the
//! aggregation rule for the protocol side — with the two differences §30 names: the unit is a
//! scenario rather than a suite, and the report carries the identity of the *specification*, which
//! an AEP conformance report has no need of.
//!
//! # A diagnostic is the product, not a by-product
//!
//! §29 requires a failure to answer five questions: what semantic rule was checked, which ESS
//! element declared it, what input was executed, what was expected, and what was observed. That is
//! not a formatting preference — the output is meant to be usable directly as repair feedback for an
//! agent, and a check whose failure message does not name the defect is barely a check.
//!
//! So [`Diagnostic`] has a field per question and none of them is optional prose. The rule sentence
//! comes from [`CheckCode`] rather than from the call site, so two failures of the same rule read the
//! same way and a reader can grep for one.

use std::collections::BTreeMap;
use std::fmt;

use aep_domain::node::Node;
use aep_domain::time::Timestamp;

use crate::scenario::{EssSemanticRef, ScenarioId, SuiteProvenance};
use crate::target::ImplementationIdentity;

// ---- status ----------------------------------------------------------------------------------

/// What one check, or one scenario, came to.
///
/// §28's four words, as one vocabulary rather than two identical ones: a scenario's status is the
/// strongest of its checks', so a second enum would be the same four names with a conversion between
/// them.
///
/// The distinction that does the work is [`Failed`](Self::Failed) against
/// [`Error`](Self::Error): the first says the implementation contradicted the specification, the
/// second says nobody found out. Collapsing them turns "your system is wrong" into "something went
/// wrong", which is the difference between a report an implementer can act on and one they have to
/// investigate.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// The specification was satisfied.
    Passed,
    /// The implementation contradicted the specification.
    Failed,
    /// The runner or the adapter could not execute the check.
    Error,
    /// The target cannot expose the observation the check requires.
    ///
    /// **Not a skip.** A required scenario in this state makes conformance fail (§28), because a
    /// suite that quietly holds fewer checks than the specification demands is the one failure a
    /// passing run cannot show.
    Unsupported,
}

impl Status {
    /// The strongest of two statuses, which is how a scenario's status comes from its checks'.
    ///
    /// The order is `Failed` > `Error` > `Unsupported` > `Passed`. A contradiction outranks
    /// everything because it is the finding; an execution failure outranks a capability gap because
    /// it hides whatever came after it, where a capability gap is a permanent property that hides
    /// nothing.
    #[must_use]
    pub fn worst(self, other: Self) -> Self {
        let rank = |status: Self| match status {
            Self::Passed => 0_u8,
            Self::Unsupported => 1,
            Self::Error => 2,
            Self::Failed => 3,
        };
        if rank(other) > rank(self) {
            other
        } else {
            self
        }
    }

    /// How it is written in a report.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Error => "error",
            Self::Unsupported => "unsupported",
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a whole run came to (§30).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ConformanceStatus {
    /// Every scenario passed.
    Passed,
    /// At least one scenario contradicted the specification, or was required and unsupported.
    Failed,
    /// Nothing contradicted the specification, and at least one scenario could not be executed.
    Error,
}

impl ConformanceStatus {
    /// How it is written.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Error => "error",
        }
    }
}

impl fmt::Display for ConformanceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---- codes -----------------------------------------------------------------------------------

/// The rule a check enforces, as a stable code and the sentence it stands for.
///
/// §29's example failure opens with `ESS-CF-OUTCOME-003`. The serial is dropped deliberately: a
/// number is a counter, and [`ScenarioId`] records at length what a counter costs when the thing it
/// numbers is inserted into. What survives is the name, which is greppable, diffable and stable
/// under every change that does not touch the rule it names — the same argument, one level down.
///
/// The code is the reason a diagnostic does not carry a hand-written rule sentence: two failures of
/// one rule read identically because the sentence lives here, once.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(into = "String", try_from = "String")]
pub enum CheckCode {
    /// A command took the branch the specification says it takes.
    Outcome,
    /// A refusing branch carried the error it declares, with the fields it declares.
    Error,
    /// A branch published an event it declares it emits.
    Event,
    /// A published event carried every field its declaration says it carries, and each is of the
    /// declared type.
    Payload,
    /// A branch published an event nothing declares it emits.
    NoEvent,
    /// A consequence the specification declares became observable within the run's deadline.
    EventualEvent,
    /// A view held what the specification says it holds.
    View,
    /// An eventual view held what the specification says it holds, within the run's deadline.
    EventualView,
    /// A binding invoked its command with the values its mapping names.
    Invocation,
    /// The identity a creating branch publishes could be read from the event that carries it.
    Instance,
    /// Every path an asserted predicate reads is published by the surface it is asserted against.
    Predicate,
    /// The target could carry out what the scenario asked of it.
    Target,
    /// The suite asked for something it cannot mean — a defect in the suite, not in the target.
    Suite,
}

impl CheckCode {
    /// Every code, which is what a meta test walks to assert each one is reachable.
    ///
    /// Public for the reason `BindingAspect::ALL` is: a list nobody iterates is a list that goes
    /// stale, and this is what makes "every rule the runner checks has a name" assertable.
    pub const ALL: [Self; 13] = [
        Self::Outcome,
        Self::Error,
        Self::Event,
        Self::Payload,
        Self::NoEvent,
        Self::EventualEvent,
        Self::View,
        Self::EventualView,
        Self::Invocation,
        Self::Instance,
        Self::Predicate,
        Self::Target,
        Self::Suite,
    ];

    /// How the code is written: `ESS-CF-OUTCOME`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Outcome => "ESS-CF-OUTCOME",
            Self::Error => "ESS-CF-ERROR",
            Self::Event => "ESS-CF-EVENT",
            Self::Payload => "ESS-CF-PAYLOAD",
            Self::NoEvent => "ESS-CF-NO-EVENT",
            Self::EventualEvent => "ESS-CF-EVENTUAL-EVENT",
            Self::View => "ESS-CF-VIEW",
            Self::EventualView => "ESS-CF-EVENTUAL-VIEW",
            Self::Invocation => "ESS-CF-INVOCATION",
            Self::Instance => "ESS-CF-INSTANCE",
            Self::Predicate => "ESS-CF-PREDICATE",
            Self::Target => "ESS-CF-TARGET",
            Self::Suite => "ESS-CF-SUITE",
        }
    }

    /// The semantic rule this code stands for, as §29's first question asks it.
    pub fn rule(self) -> &'static str {
        match self {
            Self::Outcome => "a command takes the declared branch its guards select",
            Self::Error => {
                "a refusing branch carries the error it declares, with the fields it \
                            declares"
            }
            Self::Event => "a branch publishes every event it declares it emits",
            Self::Payload => {
                "an event carries every field it declares, each holding a value of \
                              the declared type"
            }
            Self::NoEvent => "a branch publishes no event it does not declare it emits",
            Self::EventualEvent => {
                "a declared consequence becomes observable within the run's \
                                    deadline"
            }
            Self::View => {
                "a view holds what the specification says it holds, as soon as the \
                           command that changed it has returned"
            }
            Self::EventualView => {
                "an eventual view holds what the specification says it holds, \
                                   within the run's deadline"
            }
            Self::Invocation => {
                "a binding invokes its command with the value its mapping names \
                                 for each input"
            }
            Self::Instance => {
                "the identity a creating branch publishes can be read from the event \
                               that carries it"
            }
            Self::Predicate => {
                "every path an asserted predicate reads is published by the surface \
                                it is asserted against"
            }
            Self::Target => "the target can carry out what a scenario asks of it",
            Self::Suite => "a scenario asks only for what its own earlier steps established",
        }
    }

    /// Reads a code back from its written form.
    pub fn parse(value: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|code| code.as_str() == value)
            .ok_or_else(|| format!("{value:?} names no conformance check"))
    }
}

impl fmt::Display for CheckCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<CheckCode> for String {
    fn from(value: CheckCode) -> Self {
        value.as_str().to_owned()
    }
}

impl TryFrom<String> for CheckCode {
    type Error = String;

    fn try_from(value: String) -> Result<Self, String> {
        Self::parse(&value)
    }
}

// ---- diagnostics -----------------------------------------------------------------------------

/// Why a check did not pass, in the five parts §29 requires.
///
/// The rendering is §29's own layout, because the output is meant to be pasted into a repair request
/// unchanged.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Diagnostic {
    /// Which rule, as a stable code; the sentence is [`CheckCode::rule`].
    pub code: CheckCode,
    /// Which scenario was running.
    pub scenario: ScenarioId,
    /// Which ESS elements declare the rule that was checked.
    pub source: Vec<EssSemanticRef>,
    /// The command and input that were executed, where a command had been.
    pub input: Option<String>,
    /// What the specification required, one claim per line.
    pub expected: Vec<String>,
    /// What the run saw instead, one observation per line.
    pub observed: Vec<String>,
}

impl Diagnostic {
    /// A diagnostic for `code` in `scenario`, with nothing filled in yet.
    pub fn new(code: CheckCode, scenario: ScenarioId) -> Self {
        Self {
            code,
            scenario,
            source: Vec::new(),
            input: None,
            expected: Vec::new(),
            observed: Vec::new(),
        }
    }

    /// Names an ESS element that declares the rule.
    #[must_use]
    pub fn declared_by(mut self, source: impl Into<EssSemanticRef>) -> Self {
        self.source.push(source.into());
        self
    }

    /// Records the command and input that were executed.
    #[must_use]
    pub fn executing(mut self, input: impl Into<String>) -> Self {
        self.input = Some(input.into());
        self
    }

    /// Records what the specification required.
    #[must_use]
    pub fn expected(mut self, claim: impl Into<String>) -> Self {
        self.expected.push(claim.into());
        self
    }

    /// Records what was seen instead.
    #[must_use]
    pub fn observed(mut self, observation: impl Into<String>) -> Self {
        self.observed.push(observation.into());
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.code)?;
        writeln!(f)?;
        writeln!(f, "scenario:")?;
        writeln!(f, "  {}", self.scenario)?;
        writeln!(f)?;
        writeln!(f, "rule:")?;
        writeln!(f, "  {}", self.code.rule())?;
        if !self.source.is_empty() {
            writeln!(f)?;
            writeln!(f, "source:")?;
            for source in &self.source {
                writeln!(f, "  {source}")?;
            }
        }
        if let Some(input) = &self.input {
            writeln!(f)?;
            writeln!(f, "input:")?;
            writeln!(f, "  {input}")?;
        }
        writeln!(f)?;
        writeln!(f, "expected:")?;
        for claim in &self.expected {
            writeln!(f, "  {claim}")?;
        }
        writeln!(f)?;
        write!(f, "observed:")?;
        for observation in &self.observed {
            write!(f, "\n  {observation}")?;
        }
        Ok(())
    }
}

/// Renders a value the way a diagnostic quotes one.
///
/// JSON, because the workspace's dynamic value already serialises that way and a second rendering
/// would be a second thing to read. A value that cannot be written — which
/// [`Node`] permits only for a non-finite number — is named rather than dropped.
pub fn quote(value: &Node) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "<unrenderable>".to_owned())
}

/// Renders a command invocation the way §29 quotes one: `Command(field = value, …)`.
pub fn quote_input(command: &str, input: &BTreeMap<String, Node>) -> String {
    let fields = input
        .iter()
        .map(|(name, value)| format!("{name} = {}", quote(value)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{command}({fields})")
}

// ---- results ---------------------------------------------------------------------------------

/// One rule, checked once (§28).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CheckResult {
    /// Which rule.
    pub code: CheckCode,
    /// What was checked, in one line — the subject, not the rule.
    pub about: String,
    /// How it came out.
    pub status: Status,
    /// Why, when it did not pass.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<Diagnostic>,
}

impl CheckResult {
    /// A rule that holds.
    pub fn passed(code: CheckCode, about: impl Into<String>) -> Self {
        Self {
            code,
            about: about.into(),
            status: Status::Passed,
            diagnostic: None,
        }
    }

    /// A rule the implementation contradicted.
    pub fn failed(about: impl Into<String>, diagnostic: Diagnostic) -> Self {
        Self {
            code: diagnostic.code,
            about: about.into(),
            status: Status::Failed,
            diagnostic: Some(diagnostic),
        }
    }

    /// A rule nobody could check.
    pub fn errored(about: impl Into<String>, diagnostic: Diagnostic) -> Self {
        Self {
            code: diagnostic.code,
            about: about.into(),
            status: Status::Error,
            diagnostic: Some(diagnostic),
        }
    }

    /// An observation the target cannot expose.
    pub fn unsupported(about: impl Into<String>, diagnostic: Diagnostic) -> Self {
        Self {
            code: diagnostic.code,
            about: about.into(),
            status: Status::Unsupported,
            diagnostic: Some(diagnostic),
        }
    }
}

impl fmt::Display for CheckResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:<11} {} — {}", self.status, self.code, self.about)
    }
}

/// What one scenario came to (§28).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ScenarioResult {
    /// Which scenario.
    pub scenario: ScenarioId,
    /// What it proves, carried through from the suite so a report reads without one.
    pub purpose: String,
    /// The strongest status among its checks.
    pub status: Status,
    /// Every rule it checked, in the order the steps checked them.
    pub checks: Vec<CheckResult>,
    /// How long it took, in the runner's clock.
    ///
    /// Milliseconds rather than a `Duration`, because the runner's clock is
    /// [`Timestamp`]-based and a second time type would be a second answer to
    /// "when". Under an injected clock this is a function of how many times the clock was read,
    /// which is what makes two runs comparable.
    pub duration_ms: u64,
}

impl ScenarioResult {
    /// Every diagnostic this scenario produced.
    pub fn diagnostics(&self) -> impl Iterator<Item = &Diagnostic> {
        self.checks
            .iter()
            .filter_map(|check| check.diagnostic.as_ref())
    }
}

impl fmt::Display for ScenarioResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:<11} {}", self.status, self.scenario)
    }
}

/// What a run of one suite against one target came to (§30).
///
/// Carries both halves of the identity a verdict needs: which specification, through the suite's own
/// [`SuiteProvenance`], and which implementation, through [`ImplementationIdentity`]. *Conformant* is
/// a claim about one of each, and each moves independently.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ConformanceReport {
    /// Which specification, and what produced the suite.
    pub suite: SuiteProvenance,
    /// Which implementation answered.
    pub implementation: ImplementationIdentity,
    /// When the run started, in the runner's clock.
    pub started_at: Timestamp,
    /// When it finished.
    pub completed_at: Timestamp,
    /// The verdict.
    pub status: ConformanceStatus,
    /// Every scenario, in suite order.
    pub scenarios: Vec<ScenarioResult>,
}

impl ConformanceReport {
    /// The verdict a set of scenario results comes to (§28, §30).
    ///
    /// An unsupported required scenario fails the run. Every scenario a suite holds is required —
    /// the IR has no way to mark one optional — so there is no exception to apply here, and adding
    /// one would be adding the silent skip §28 forbids.
    pub fn verdict(scenarios: &[ScenarioResult]) -> ConformanceStatus {
        if scenarios
            .iter()
            .any(|result| matches!(result.status, Status::Failed | Status::Unsupported))
        {
            ConformanceStatus::Failed
        } else if scenarios
            .iter()
            .any(|result| result.status == Status::Error)
        {
            ConformanceStatus::Error
        } else {
            ConformanceStatus::Passed
        }
    }

    /// `true` only when every scenario passed.
    pub fn is_conformant(&self) -> bool {
        self.status == ConformanceStatus::Passed
    }

    /// How many scenarios came to each status.
    pub fn counts(&self) -> BTreeMap<Status, usize> {
        let mut counts = BTreeMap::new();
        for result in &self.scenarios {
            *counts.entry(result.status).or_insert(0) += 1;
        }
        counts
    }

    /// The scenarios that did not pass.
    pub fn failures(&self) -> impl Iterator<Item = &ScenarioResult> {
        self.scenarios
            .iter()
            .filter(|result| result.status != Status::Passed)
    }

    /// Every diagnostic the run produced, in scenario order.
    pub fn diagnostics(&self) -> impl Iterator<Item = &Diagnostic> {
        self.scenarios.iter().flat_map(ScenarioResult::diagnostics)
    }

    /// The report as canonical JSON, with a trailing newline.
    ///
    /// The same three properties [`ConformanceSuite::to_canonical_json`](crate::ConformanceSuite::to_canonical_json)
    /// means by canonical, and for a further reason: under an injected clock and id source, two runs
    /// produce byte-identical output, which is what makes a committed report reviewable by diff.
    ///
    /// # Panics
    ///
    /// It does not. Every map here is keyed by a string or by a type that serialises as one.
    pub fn to_canonical_json(&self) -> String {
        let mut json = serde_json::to_string_pretty(self)
            .unwrap_or_else(|error| panic!("a conformance report serialises: {error}"));
        json.push('\n');
        json
    }
}

impl fmt::Display for ConformanceReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{} {} against {} — {}",
            self.suite.system, self.suite.specification_version, self.implementation, self.status
        )?;
        for result in &self.scenarios {
            writeln!(f, "  {result}")?;
        }
        let counts = self.counts();
        write!(f, "  {} scenarios: ", self.scenarios.len())?;
        let summary = [
            Status::Passed,
            Status::Failed,
            Status::Error,
            Status::Unsupported,
        ]
        .into_iter()
        .map(|status| format!("{} {status}", counts.get(&status).copied().unwrap_or(0)))
        .collect::<Vec<_>>()
        .join(", ");
        writeln!(f, "{summary}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::{CommandRef, OutcomeRef};
    use ess_domain::command::OutcomeName;
    use ess_domain::name::QualifiedName;

    fn scenario() -> ScenarioId {
        ScenarioId::Outcome { outcome: outcome() }
    }

    fn outcome() -> OutcomeRef {
        OutcomeRef::new(
            CommandRef::new(QualifiedName::new("billing.invoice.CreateInvoice").expect("valid")),
            OutcomeName::new("rejected").expect("valid"),
        )
    }

    #[test]
    fn every_check_code_has_a_distinct_name_and_a_rule_sentence() {
        // `ALL` is what a meta test walks, so a code missing from it is a rule nothing can be
        // grouped by and nothing can assert is reachable.
        let mut seen = std::collections::BTreeSet::new();
        for code in CheckCode::ALL {
            assert!(
                code.as_str().starts_with("ESS-CF-"),
                "{code} is not written the way §29 writes one"
            );
            assert!(
                seen.insert(code.as_str()),
                "{code} shares its name with another code"
            );
            assert!(
                !code.rule().is_empty(),
                "{code} has no sentence, so a diagnostic of it says which rule only by its acronym"
            );
            assert_eq!(CheckCode::parse(code.as_str()), Ok(code));
        }
        assert_eq!(seen.len(), CheckCode::ALL.len());
        CheckCode::parse("ESS-CF-NOPE").expect_err("names no check");
    }

    #[test]
    fn a_scenario_status_is_the_strongest_of_its_checks_and_a_contradiction_outranks_everything() {
        // The fixture holds all four before the rule is asked about it, because a precedence rule
        // checked against two values proves nothing about the third.
        let all = [
            Status::Passed,
            Status::Unsupported,
            Status::Error,
            Status::Failed,
        ];
        for status in all {
            assert_eq!(status.worst(status), status);
            assert_eq!(
                Status::Passed.worst(status),
                status,
                "passing must never mask {status}"
            );
        }
        assert_eq!(Status::Failed.worst(Status::Error), Status::Failed);
        assert_eq!(Status::Error.worst(Status::Failed), Status::Failed);
        assert_eq!(Status::Error.worst(Status::Unsupported), Status::Error);
        assert_eq!(
            Status::Unsupported.worst(Status::Passed),
            Status::Unsupported
        );
    }

    #[test]
    fn an_unsupported_scenario_makes_the_run_fail_rather_than_look_like_a_pass() {
        let result = |status: Status| ScenarioResult {
            scenario: scenario(),
            purpose: "a positive amount is accepted".to_owned(),
            status,
            checks: Vec::new(),
            duration_ms: 0,
        };

        assert_eq!(
            ConformanceReport::verdict(&[result(Status::Passed)]),
            ConformanceStatus::Passed
        );
        assert_eq!(
            ConformanceReport::verdict(&[result(Status::Passed), result(Status::Unsupported)]),
            ConformanceStatus::Failed,
            "§28: an unsupported required scenario makes conformance fail"
        );
        assert_eq!(
            ConformanceReport::verdict(&[result(Status::Passed), result(Status::Error)]),
            ConformanceStatus::Error
        );
        assert_eq!(
            ConformanceReport::verdict(&[result(Status::Error), result(Status::Failed)]),
            ConformanceStatus::Failed,
            "a contradiction found is the headline, not the check that could not run"
        );
    }

    #[test]
    fn a_diagnostic_answers_all_five_of_the_questions_a_failure_has_to_answer() {
        let diagnostic = Diagnostic::new(CheckCode::Outcome, scenario())
            .declared_by(outcome())
            .executing("billing.invoice.CreateInvoice(amount = 0)")
            .expected("outcome = rejected")
            .observed("outcome = accepted");

        let rendered = diagnostic.to_string();
        for required in [
            "ESS-CF-OUTCOME",
            "billing.invoice.CreateInvoice/outcome/rejected",
            CheckCode::Outcome.rule(),
            "outcome billing.invoice.CreateInvoice/rejected",
            "billing.invoice.CreateInvoice(amount = 0)",
            "outcome = rejected",
            "outcome = accepted",
        ] {
            assert!(
                rendered.contains(required),
                "a §29 diagnostic names {required:?}; it read:\n{rendered}"
            );
        }
    }

    #[test]
    fn a_quoted_input_reads_as_the_call_that_was_made() {
        let mut input = BTreeMap::new();
        input.insert("amount".to_owned(), Node::Text("0".to_owned()));
        assert_eq!(
            quote_input("billing.invoice.CreateInvoice", &input),
            r#"billing.invoice.CreateInvoice(amount = "0")"#
        );
    }
}
