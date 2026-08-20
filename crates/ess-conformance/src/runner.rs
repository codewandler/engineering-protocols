//! Executing a suite against a target.
//!
//! Design §27. The runner owns scenario sequencing, isolation, bounded waiting, comparison,
//! diagnostics and report assembly; the target owns invoking the implementation, observing what the
//! specification declares observable, mapping its own failures into [`TargetError`], and waiting
//! until it can satisfy a consistency requirement.
//!
//! # The runner owns every source of variation
//!
//! §37, and it is the stricter of the two splits. A [`Runner`] is **constructed with** its clock and
//! its id source, and nothing below it reaches for an ambient one: no `SystemTime::now`, no RNG, no
//! global. `aep_conformance::harness::Harness` is the worked example the design points at — a
//! monotonic sequence counter and a clock starting at a fixed instant, both held by the harness.
//!
//! [`Runner::run`] takes `self` by value for the same reason. Running twice means constructing
//! twice, so "the same runner produces the same report" is not a discipline anybody has to keep: two
//! identically-constructed runners cannot diverge, because neither carries anything the other's
//! previous run moved.
//!
//! # Nothing sleeps
//!
//! §40, and §15 was corrected to agree with it: the runner configures a **deadline** and nothing
//! else about waiting. There is no poll interval, because a poll interval is a fixed delay spelled
//! as configuration — the same test of the machine it runs on, repeated in a loop.
//!
//! An [`EventuallyEvent`](crate::scenario::ScenarioStep::EventuallyEvent) or
//! [`EventuallyView`](crate::scenario::ScenarioStep::EventuallyView) asks the target again, hands it
//! the deadline, and stops when the runner's own clock has passed it. Because [`Clock::now`]
//! advances on every read, the number of asks is a function of the configured budget and the clock's
//! step — a bounded number of semantic queries, which is exactly what §40 means by bounded polling —
//! and the *waiting* happens inside the target, the only layer that knows what it is waiting for.
//!
//! # Immediate or eventual is decided by the step, never re-derived
//!
//! `ResolvedView::assertion_style` already decided it once, from the view's declared consistency,
//! and synthesis wrote that decision into the shape of the scenario. So the runner reads it off the
//! step and never looks at a consistency word again:
//!
//! | step | consistency demanded | why |
//! |---|---|---|
//! | [`QueryView`](crate::scenario::ScenarioStep::QueryView) | `AtLeast(token)` from the last command, or `Current` when no command has run | §14: a `read_your_writes` view must show the command that just returned, and the target blocks until it can |
//! | [`EventuallyView`](crate::scenario::ScenarioStep::EventuallyView) | `Current`, asked again until the deadline | demanding `AtLeast(token)` of a projection would make the target block until it caught up, which is the assertion this step exists to make |
//!
//! # What stops a scenario and what does not
//!
//! A failed expectation does not stop it. `CreateInvoice` that answers the wrong branch *and*
//! publishes the wrong event is two findings, and reporting one of them makes the second look like a
//! consequence of the first. Validation accumulates here for the reason it accumulates everywhere
//! else in this workspace.
//!
//! An `error` does stop it, because a step that could not run leaves every later step reading state
//! that was never established — an unbound instance, a command result that does not exist — and a
//! cascade of errors buries the one that matters.

use std::collections::BTreeMap;

use aep_contract::consistency::QueryConsistency;
use aep_domain::facts::{FactPath, FactStore, FactValue};
use aep_domain::ids::CorrelationId;
use aep_domain::node::Node;
use aep_domain::predicate::{Predicate, Truth};
use aep_domain::time::Timestamp;

use crate::report::{
    quote, quote_input, CheckCode, CheckResult, ConformanceReport, Diagnostic, ScenarioResult,
    Status,
};
use crate::scenario::{
    ActorRef, BindingRef, CommandRef, ConformanceScenario, ConformanceSuite, EntityRef, ErrorRef,
    EventRef, InstanceName, OutcomeRef, PayloadShape, ScenarioId, ScenarioStep, ScenarioValue,
    ViewExpectation, ViewRef,
};
use crate::target::{
    ConformanceTarget, Deadline, EventObservationRequest, ExternalOutcomeControl,
    ImplementationIdentity, InvocationObservationRequest, ObservedEvent, RedeliveryRequest,
    ScenarioContext, SemanticCommandRequest, SemanticCommandResult, SemanticViewRequest,
    SemanticViewResult, TargetError, ViewRow,
};

// ---- the clock -------------------------------------------------------------------------------

/// The runner's source of time.
///
/// # Why this is not `aep_engine::clock::Clock`
///
/// The same idea, and deliberately not the same trait, for two reasons that are both about what the
/// runner needs rather than about taste.
///
/// **It must advance.** A deadline bounds an eventual assertion only if reading the clock moves it;
/// `aep_engine::clock::FixedClock` satisfies that trait by never moving, and a runner handed one
/// would ask its target forever. So [`now`](Clock::now) takes `&mut self`: a clock that must advance
/// is a clock that mutates, and the type says so.
///
/// **The dependency is the wrong shape.** `aep-engine` is principle resolution and workflow
/// evaluation; importing it here would pull the AEP workflow engine and `aep-schema` into the ESS
/// stack for a three-line trait. This crate depends on `aep-domain` for values and on `aep-contract`
/// for the consistency pair §14 requires it to reuse, and on nothing that evaluates AEP workflows.
///
/// The refusal is recorded rather than assumed, as this repository asks: reusing the AEP trait was
/// weighed, and what it would have cost is admitting a clock that makes `Eventually*` non-terminating.
pub trait Clock {
    /// The current time, advanced by this read.
    fn now(&mut self) -> Timestamp;
}

/// A clock that starts at a fixed instant and advances by a fixed step on every read.
///
/// The shape §37 asks for and `aep_conformance::harness::Harness` already uses: "a timestamp that
/// advances by a second per call, so ordering is observable without sleeping". Under one of these a
/// run is reproducible — the report's `started_at`, every `duration_ms` and the number of times a
/// bounded assertion asked are all functions of how many times the clock was read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvancingClock {
    next: u64,
    step: u64,
}

impl AdvancingClock {
    /// The instant the default clock starts at: 2023-11-14T22:13:20Z, as
    /// `aep_conformance::harness::Harness` uses.
    pub const DEFAULT_START_MS: u64 = 1_700_000_000_000;

    /// How far the default clock moves per read.
    ///
    /// With [`RunnerConfig::DEFAULT_EVENTUAL_TIMEOUT_MS`] this bounds an eventual assertion at fifty
    /// asks, which is a budget rather than a delay: no wall-clock time passes.
    pub const DEFAULT_STEP_MS: u64 = 100;

    /// A clock starting at `start_ms` and advancing `step_ms` per read.
    ///
    /// A step of zero is raised to one. A clock that does not move cannot bound anything, and
    /// silently accepting one would trade a compile-time guarantee for a hang.
    pub fn new(start_ms: u64, step_ms: u64) -> Self {
        Self {
            next: start_ms,
            step: step_ms.max(1),
        }
    }
}

impl Default for AdvancingClock {
    fn default() -> Self {
        Self::new(Self::DEFAULT_START_MS, Self::DEFAULT_STEP_MS)
    }
}

impl Clock for AdvancingClock {
    fn now(&mut self) -> Timestamp {
        let now = self.next;
        self.next = self.next.saturating_add(self.step);
        Timestamp::from_epoch_millis(now)
    }
}

// ---- the id source ---------------------------------------------------------------------------

/// The runner's source of correlation ids: a monotonic counter, seeded from the suite (§37).
///
/// Seeded from the suite rather than from a wall clock or a random device, so that the ids in a
/// report are a function of what was run. The target is *given* these; a target that minted its own
/// would put a value in the report that differs between two runs for no semantic reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ids {
    prefix: String,
    next: u64,
}

impl Ids {
    /// An id source seeded from a suite's provenance.
    pub fn for_suite(suite: &ConformanceSuite) -> Self {
        let seed = format!(
            "{}-{}",
            suite.provenance.system, suite.provenance.spec_digest
        );
        Self::seeded(seed)
    }

    /// An id source seeded from an arbitrary label.
    ///
    /// The label is reduced to what a `CorrelationId` accepts, because a system name is an ESS name
    /// and a correlation id is an AEP identifier, and the two charsets are not the same rule.
    pub fn seeded(seed: impl AsRef<str>) -> Self {
        let mut prefix = String::new();
        for character in seed.as_ref().chars() {
            if character.is_ascii_alphanumeric() {
                prefix.push(character);
            } else if !prefix.ends_with('-') {
                prefix.push('-');
            }
        }
        let prefix = prefix.trim_matches('-').to_owned();
        Self {
            prefix: if prefix.is_empty() {
                "ess".to_owned()
            } else {
                prefix
            },
            next: 0,
        }
    }

    /// The next correlation id.
    ///
    /// # Panics
    ///
    /// It does not. The prefix is reduced to ASCII alphanumerics and single hyphens and is never
    /// empty, and the suffix is decimal digits, so the result always satisfies `CorrelationId`.
    pub fn correlation(&mut self) -> CorrelationId {
        self.next += 1;
        let rendered = format!("{}-{:06}", self.prefix, self.next);
        CorrelationId::new(rendered.clone())
            .unwrap_or_else(|error| panic!("a generated correlation id is well formed: {error}"))
    }
}

// ---- configuration ---------------------------------------------------------------------------

/// What the execution environment decides, which is a deadline and nothing else (§15).
///
/// Deliberately not a poll interval. §15 configured one in an earlier draft and §40 forbids exactly
/// that; the contradiction was resolved in favour of §40. These values affect test execution, not
/// the meaning of the specification: if a specification ever models a true semantic time bound —
/// "the event must occur within two seconds" — that is a different concept and belongs in the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerConfig {
    /// How long a bounded observation may go on being asked for, in the runner's clock.
    pub eventual_timeout_ms: u64,
}

impl RunnerConfig {
    /// The default budget: five seconds of the runner's clock, as §15's example configures.
    pub const DEFAULT_EVENTUAL_TIMEOUT_MS: u64 = 5_000;

    /// A configuration with a budget of `eventual_timeout_ms`.
    pub fn new(eventual_timeout_ms: u64) -> Self {
        Self {
            eventual_timeout_ms,
        }
    }
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self::new(Self::DEFAULT_EVENTUAL_TIMEOUT_MS)
    }
}

// ---- the runner ------------------------------------------------------------------------------

/// Executes a suite against a target and reports what it found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Runner<C: Clock = AdvancingClock> {
    config: RunnerConfig,
    clock: C,
    ids: Ids,
}

impl Runner<AdvancingClock> {
    /// A runner over the default clock and a budget of
    /// [`RunnerConfig::DEFAULT_EVENTUAL_TIMEOUT_MS`], with ids seeded from `suite`.
    ///
    /// Two runners built this way from one suite are equal, which is the whole claim: they cannot
    /// produce different reports against a deterministic target.
    pub fn for_suite(suite: &ConformanceSuite) -> Self {
        Self::new(
            RunnerConfig::default(),
            AdvancingClock::default(),
            Ids::for_suite(suite),
        )
    }
}

impl<C: Clock> Runner<C> {
    /// A runner over an explicit configuration, clock and id source.
    pub fn new(config: RunnerConfig, clock: C, ids: Ids) -> Self {
        Self { config, clock, ids }
    }

    /// Runs every scenario in `suite` against `target`, in id order.
    ///
    /// Takes `self` by value: a second run means a second runner, so no run can inherit the clock or
    /// the counter the previous one left behind.
    pub fn run<T: ConformanceTarget>(
        mut self,
        suite: &ConformanceSuite,
        target: &T,
    ) -> ConformanceReport {
        let started_at = self.clock.now();
        let implementation = target
            .identity()
            .unwrap_or_else(|error| ImplementationIdentity::new("unidentified", error.to_string()));

        let mut scenarios = Vec::with_capacity(suite.len());
        for (id, scenario) in &suite.scenarios {
            scenarios.push(self.scenario(id, scenario, target));
        }

        let completed_at = self.clock.now();
        ConformanceReport {
            suite: suite.provenance.clone(),
            implementation,
            started_at,
            completed_at,
            status: ConformanceReport::verdict(&scenarios),
            scenarios,
        }
    }

    /// Runs one scenario in its own execution context.
    fn scenario<T: ConformanceTarget>(
        &mut self,
        id: &ScenarioId,
        scenario: &ConformanceScenario,
        target: &T,
    ) -> ScenarioResult {
        let started = self.clock.now();
        let context = ScenarioContext::new(id.clone(), self.ids.correlation());
        let mut run = Run::new(id.clone(), context);

        if let Err(error) = target.begin_scenario(&run.context) {
            run.record(target_failure(
                &run.id,
                "opening an isolated execution context",
                &error,
            ));
        } else {
            for step in &scenario.steps {
                if self.step(step, &mut run, target) == Flow::Stop {
                    break;
                }
            }
            if let Err(error) = target.end_scenario(&run.context) {
                run.record(target_failure(
                    &run.id,
                    "closing the execution context",
                    &error,
                ));
            }
        }

        let status = run
            .checks
            .iter()
            .fold(Status::Passed, |worst, check| worst.worst(check.status));
        ScenarioResult {
            scenario: id.clone(),
            purpose: scenario.purpose.to_string(),
            status,
            checks: run.checks,
            duration_ms: self
                .clock
                .now()
                .epoch_millis()
                .saturating_sub(started.epoch_millis()),
        }
    }

    /// The instant a bounded observation started now would give up at.
    fn deadline(&mut self) -> Deadline {
        Deadline::at(Timestamp::from_epoch_millis(
            self.clock
                .now()
                .epoch_millis()
                .saturating_add(self.config.eventual_timeout_ms),
        ))
    }

    /// Executes one step.
    ///
    /// A dispatcher and nothing more. Each of the thirteen steps is its own function, so the rule a
    /// step enforces and the diagnostic it produces sit together and can be read without the other
    /// twelve — and so that adding a step to the vocabulary is a decision that has somewhere to go.
    fn step<T: ConformanceTarget>(
        &mut self,
        step: &ScenarioStep,
        run: &mut Run,
        target: &T,
    ) -> Flow {
        match step {
            ScenarioStep::ConfigureExternalOutcome { force } => {
                configure_external(force, run, target)
            }
            ScenarioStep::ExecuteCommand {
                command,
                actor,
                input,
            } => execute_command(command, actor.as_ref(), input, run, target),
            ScenarioStep::ExpectOutcome { outcome } => expect_outcome(outcome, run),
            ScenarioStep::ExpectError { error, fields } => expect_error(error, fields, run),
            ScenarioStep::ExpectEvent {
                event,
                payload,
                shape,
            } => expect_event(event, payload, shape, run),
            ScenarioStep::ExpectNoEvent { event } => expect_no_event(event, run),
            ScenarioStep::CaptureInstance {
                instance,
                entity,
                event,
                field,
            } => capture_instance(instance, entity, event, field, run),
            ScenarioStep::RedeliverEvent { event } => redeliver_event(event, run, target),
            ScenarioStep::ExpectInvocation {
                binding,
                command,
                input,
            } => self.expect_invocation(binding, command, input, run, target),
            ScenarioStep::QueryView { view } => self.query_view(view, run, target),
            ScenarioStep::ExpectView { view, expectation } => expect_view(view, expectation, run),
            ScenarioStep::EventuallyEvent {
                event,
                payload,
                shape,
            } => self.eventually_event(event, payload, shape, run, target),
            ScenarioStep::EventuallyView { view, expectation } => {
                self.eventually_view(view, expectation, run, target)
            }
        }
    }

    /// Requires that a binding invoked its command with the values its mapping names (§16).
    ///
    /// The one step whose absence from a target is not a failure: §16 refuses to require command
    /// tracing of every implementation, so [`TargetError::Unsupported`] here is recorded as
    /// `unsupported` — which still makes conformance fail (§28), and still says which single
    /// scenario the target could not answer.
    fn expect_invocation<T: ConformanceTarget>(
        &mut self,
        binding: &BindingRef,
        command: &CommandRef,
        input: &BTreeMap<String, ScenarioValue>,
        run: &mut Run,
        target: &T,
    ) -> Flow {
        let mut wanted = BTreeMap::new();
        for (field, value) in input {
            match run.resolve(value) {
                Ok(node) => {
                    wanted.insert(field.clone(), node);
                }
                Err(reason) => {
                    run.record(CheckResult::errored(
                        format!("the mapping of `{field}` by `{binding}`"),
                        Diagnostic::new(CheckCode::Suite, run.id.clone())
                            .declared_by(binding.clone())
                            .expected(format!(
                                "`{field}` refers to something an earlier step observed"
                            ))
                            .observed(reason),
                    ));
                    return Flow::Stop;
                }
            }
        }
        let deadline = self.deadline();
        let request = InvocationObservationRequest {
            binding: binding.clone(),
            command: command.clone(),
            correlation: run.context.correlation.clone(),
            deadline,
        };
        let about = format!("the mapping `{binding}` applies to `{command}`");
        match target.observe_invocations(request) {
            Ok(invocations) => {
                let found = invocations.iter().any(|invocation| {
                    &invocation.command == command && matches(&invocation.input, &wanted)
                });
                if found {
                    run.record(CheckResult::passed(CheckCode::Invocation, about));
                } else {
                    let mut diagnostic = Diagnostic::new(CheckCode::Invocation, run.id.clone())
                        .declared_by(binding.clone())
                        .declared_by(command.clone());
                    for (field, value) in &wanted {
                        diagnostic =
                            diagnostic.expected(format!("{command}.{field} = {}", quote(value)));
                    }
                    let seen = if invocations.is_empty() {
                        format!("`{binding}` invoked `{command}` no times")
                    } else {
                        invocations
                            .iter()
                            .map(|invocation| {
                                quote_input(&invocation.command.to_string(), &invocation.input)
                            })
                            .collect::<Vec<_>>()
                            .join("; ")
                    };
                    run.record(CheckResult::failed(about, diagnostic.observed(seen)));
                }
                Flow::Continue
            }
            Err(error) if error.is_unsupported() => {
                run.record(CheckResult::unsupported(
                    about,
                    Diagnostic::new(CheckCode::Invocation, run.id.clone())
                        .declared_by(binding.clone())
                        .declared_by(command.clone())
                        .expected("the target exposes the commands its bindings invoke (§16)")
                        .observed(error.to_string()),
                ));
                Flow::Continue
            }
            Err(error) => {
                run.record(target_failure(
                    &run.id,
                    &format!("observing what `{binding}` invoked"),
                    &error,
                ));
                Flow::Stop
            }
        }
    }

    /// Reads a view, no older than the command that just returned (§14).
    ///
    /// The consistency is not a choice made here: a `read_your_writes` view is the only kind
    /// synthesis writes this step for, and demanding `AtLeast(token)` is what makes the target — not
    /// the runner — responsible for waiting until it can answer.
    ///
    /// # A command that returns no token is not read at `Current` instead
    ///
    /// It would be the easy thing, and it is the "skip that looks like a pass". §14's whole claim is
    /// that the query sees the command that just returned; with no token in hand, `Current` asks a
    /// weaker question, gets a green tick and reports a check that was never made. So the read is
    /// **not** issued, and [`expect_view`] records what actually happened — see there for why that
    /// is `failed` rather than `unsupported`. The target owns satisfying `AtLeast(T)`; a target that
    /// will not say what `T` is has not left it anything to satisfy.
    fn query_view<T: ConformanceTarget>(
        &mut self,
        view: &ViewRef,
        run: &mut Run,
        target: &T,
    ) -> Flow {
        let consistency = match run.last_command.as_ref() {
            // Nothing has been written in this scenario, so there is no write to read no older
            // than. Synthesis never produces this, and a suite that does means it.
            None => QueryConsistency::Current,
            Some(executed) => {
                let Some(token) = executed.result.consistency.clone() else {
                    run.unreadable = Some((view.clone(), executed.command.clone()));
                    run.last_view = None;
                    return Flow::Continue;
                };
                QueryConsistency::at_least(token)
            }
        };
        let deadline = self.deadline();
        let request = SemanticViewRequest {
            view: view.clone(),
            consistency,
            correlation: run.context.correlation.clone(),
            deadline,
        };
        match target.query_view(request) {
            Ok(result) => {
                run.unreadable = None;
                run.last_view = Some((view.clone(), result));
                Flow::Continue
            }
            Err(error) => {
                run.record(target_failure(
                    &run.id,
                    &format!("reading `{view}`"),
                    &error,
                ));
                Flow::Stop
            }
        }
    }

    /// Asks for an event until it is observed or the deadline has passed (§15, §40).
    fn eventually_event<T: ConformanceTarget>(
        &mut self,
        event: &EventRef,
        payload: &BTreeMap<String, Node>,
        shape: &PayloadShape,
        run: &mut Run,
        target: &T,
    ) -> Flow {
        let deadline = self.deadline();
        let mut asks = 0_u32;
        loop {
            asks += 1;
            let request = EventObservationRequest {
                event: event.clone(),
                correlation: run.context.correlation.clone(),
                deadline,
            };
            let observed = match target.observe_events(request) {
                Ok(observed) => observed,
                Err(error) => {
                    run.record(target_failure(
                        &run.id,
                        &format!("observing `{event}`"),
                        &error,
                    ));
                    return Flow::Stop;
                }
            };
            run.remember(&observed);
            let carried = observed
                .iter()
                .find(|seen| &seen.event == event && matches(&seen.payload, payload))
                .map(|seen| seen.payload.clone());
            if let Some(carried) = carried {
                run.record(CheckResult::passed(
                    CheckCode::EventualEvent,
                    format!("event {event}, eventually"),
                ));
                // The occurrence that arrived is held to the same declaration an immediate one is:
                // crossing a component boundary changes when a consequence is observable, not what
                // it carries.
                let executing = run.last_command.as_ref().map(Executed::quoted);
                expect_payload(event, &carried, shape, executing, run);
                return Flow::Continue;
            }
            if deadline.has_passed(self.clock.now()) {
                let mut diagnostic = Diagnostic::new(CheckCode::EventualEvent, run.id.clone())
                    .declared_by(event.clone())
                    .expected(format!("event {event} observed within the run's deadline"));
                for (field, value) in payload {
                    diagnostic = diagnostic.expected(format!("{event}.{field} = {}", quote(value)));
                }
                if let Some(executed) = run.last_command.as_ref() {
                    diagnostic = diagnostic.executing(executed.quoted());
                }
                run.record(CheckResult::failed(
                    format!("event {event}, eventually"),
                    diagnostic.observed(format!(
                        "not observed after {asks} observations; the target reported {}",
                        run.summary()
                    )),
                ));
                return Flow::Continue;
            }
        }
    }

    /// Reads a view until it satisfies the expectation or the deadline has passed (§14, §40).
    fn eventually_view<T: ConformanceTarget>(
        &mut self,
        view: &ViewRef,
        expectation: &ViewExpectation,
        run: &mut Run,
        target: &T,
    ) -> Flow {
        let required = match Required::of(expectation, run) {
            Ok(required) => required,
            Err(reason) => {
                run.record(unresolvable(view, &run.id, &reason));
                return Flow::Stop;
            }
        };
        let deadline = self.deadline();
        let mut asks = 0_u32;
        loop {
            asks += 1;
            let request = SemanticViewRequest {
                view: view.clone(),
                // `Current`, never `AtLeast`: demanding the token would make the target block until
                // the projection caught up, which is the very thing this step exists to observe.
                consistency: QueryConsistency::Current,
                correlation: run.context.correlation.clone(),
                deadline,
            };
            let result = match target.query_view(request) {
                Ok(result) => result,
                Err(error) => {
                    run.record(target_failure(
                        &run.id,
                        &format!("reading `{view}`"),
                        &error,
                    ));
                    return Flow::Stop;
                }
            };
            let verdict = decide(&required, &result);
            match verdict {
                Verdict::Satisfied | Verdict::Undecidable { .. } => {
                    run.last_view = Some((view.clone(), result));
                    run.record(view_check(
                        CheckCode::EventualView,
                        &run.id,
                        view,
                        &required,
                        &verdict,
                    ));
                    return Flow::Continue;
                }
                Verdict::Unsatisfied(_) if deadline.has_passed(self.clock.now()) => {
                    run.last_view = Some((view.clone(), result));
                    let mut check =
                        view_check(CheckCode::EventualView, &run.id, view, &required, &verdict);
                    if let Some(diagnostic) = check.diagnostic.take() {
                        check.diagnostic =
                            Some(diagnostic.observed(format!("still so after {asks} reads")));
                    }
                    run.record(check);
                    return Flow::Continue;
                }
                Verdict::Unsatisfied(_) => {}
            }
        }
    }
}

// ---- the steps that need no clock --------------------------------------------------------------

/// Forces the next answer of an outcome the input cannot decide (§12).
fn configure_external<T: ConformanceTarget>(force: &OutcomeRef, run: &mut Run, target: &T) -> Flow {
    let request = ExternalOutcomeControl {
        force: force.clone(),
        correlation: run.context.correlation.clone(),
    };
    match target.configure_external_outcome(request) {
        Ok(()) => Flow::Continue,
        Err(error) => {
            run.record(target_failure(
                &run.id,
                &format!("forcing the external outcome `{force}`"),
                &error,
            ));
            Flow::Stop
        }
    }
}

/// Invokes a command, with every reference the suite carries resolved first (§9).
fn execute_command<T: ConformanceTarget>(
    command: &CommandRef,
    actor: Option<&ActorRef>,
    input: &BTreeMap<String, ScenarioValue>,
    run: &mut Run,
    target: &T,
) -> Flow {
    let mut resolved = BTreeMap::new();
    for (field, value) in input {
        match run.resolve(value) {
            Ok(node) => {
                resolved.insert(field.clone(), node);
            }
            Err(reason) => {
                run.record(CheckResult::errored(
                    format!("the input `{field}` of `{command}`"),
                    Diagnostic::new(CheckCode::Suite, run.id.clone())
                        .declared_by(command.clone())
                        .expected(format!(
                            "`{field}` refers to something an earlier step established"
                        ))
                        .observed(reason),
                ));
                return Flow::Stop;
            }
        }
    }
    let request = SemanticCommandRequest {
        command: command.clone(),
        actor: actor.cloned(),
        input: resolved.clone(),
        correlation: run.context.correlation.clone(),
    };
    match target.execute_command(request) {
        Ok(result) => {
            run.remember(&result.direct_events);
            run.last_command = Some(Executed {
                command: command.to_string(),
                input: resolved,
                result,
            });
            Flow::Continue
        }
        Err(error) => {
            run.record(target_failure(
                &run.id,
                &format!("invoking `{command}`"),
                &error,
            ));
            Flow::Stop
        }
    }
}

/// Requires that the last command took the branch the specification says it takes (§10).
fn expect_outcome(outcome: &OutcomeRef, run: &mut Run) -> Flow {
    let Some(executed) = run.last_command.as_ref() else {
        run.record(no_command(&run.id, "an outcome"));
        return Flow::Stop;
    };
    let about = format!("outcome {outcome}");
    if executed.result.outcome.as_ref() == Some(outcome) {
        run.record(CheckResult::passed(CheckCode::Outcome, about));
    } else {
        let seen = executed.result.outcome.as_ref().map_or_else(
            || {
                "no declared outcome was reached; the target refused for a reason the \
                 specification does not model"
                    .to_owned()
            },
            |reached| format!("outcome = {}", reached.outcome),
        );
        let diagnostic = Diagnostic::new(CheckCode::Outcome, run.id.clone())
            .declared_by(outcome.clone())
            .executing(executed.quoted())
            .expected(format!("outcome = {}", outcome.outcome))
            .observed(seen);
        run.record(CheckResult::failed(about, diagnostic));
    }
    Flow::Continue
}

/// Requires the declared error a refusing branch carries, and the fields the suite names of it.
fn expect_error(error: &ErrorRef, fields: &BTreeMap<String, Node>, run: &mut Run) -> Flow {
    let Some(executed) = run.last_command.as_ref() else {
        run.record(no_command(&run.id, "a declared error"));
        return Flow::Stop;
    };
    let mut diagnostic = Diagnostic::new(CheckCode::Error, run.id.clone())
        .declared_by(error.clone())
        .executing(executed.quoted())
        .expected(format!("error = {error}"));
    for (field, value) in fields {
        diagnostic = diagnostic.expected(format!("error.{field} = {}", quote(value)));
    }
    let carried = match executed.result.error.as_ref() {
        None => Some("no declared error was carried".to_owned()),
        Some(value) if &value.error != error => Some(format!("error = {}", value.error)),
        Some(value) => mismatch("error", &value.fields, fields),
    };
    let about = format!("error {error}");
    match carried {
        None => run.record(CheckResult::passed(CheckCode::Error, about)),
        Some(seen) => run.record(CheckResult::failed(about, diagnostic.observed(seen))),
    }
    Flow::Continue
}

/// Requires that the last command published an event it declares it emits (§13).
fn expect_event(
    event: &EventRef,
    payload: &BTreeMap<String, Node>,
    shape: &PayloadShape,
    run: &mut Run,
) -> Flow {
    let Some(executed) = run.last_command.as_ref() else {
        run.record(no_command(&run.id, "an event"));
        return Flow::Stop;
    };
    let about = format!("event {event}");
    let found = executed
        .result
        .direct_events
        .iter()
        .find(|observed| &observed.event == event && matches(&observed.payload, payload))
        .map(|observed| observed.payload.clone());
    let Some(carried) = found else {
        let mut diagnostic = Diagnostic::new(CheckCode::Event, run.id.clone())
            .declared_by(event.clone())
            .executing(executed.quoted())
            .expected(format!("event {event} published"));
        for (field, value) in payload {
            diagnostic = diagnostic.expected(format!("{event}.{field} = {}", quote(value)));
        }
        let observed = published(&executed.result);
        run.record(CheckResult::failed(about, diagnostic.observed(observed)));
        return Flow::Continue;
    };
    let executing = executed.quoted();
    run.record(CheckResult::passed(CheckCode::Event, about));
    // A separate check, and separately named: "the event happened" and "the event carried what it
    // declares" are two rules, and a report that folded them into one would say `ESS-CF-EVENT`
    // failed for a system that published exactly the right event with half a payload.
    expect_payload(event, &carried, shape, Some(executing), run);
    Flow::Continue
}

/// Requires that an occurrence carried every field it declares, each of the declared type (§13).
///
/// # What is not checked here, and why
///
/// A **value**. `PayloadShape` argues it in full: the model relates a command's input to no payload
/// field, so `InvoiceCreated.amount == CreateInvoice.amount` is a name match rather than a reading,
/// and a suite that asserted it would fail an implementation that is doing nothing wrong.
///
/// An **undeclared** field, for the mirror-image reason: nothing in the model closes an event's
/// payload, so refusing an extra field would enforce a rule no document wrote.
fn expect_payload(
    event: &EventRef,
    carried: &BTreeMap<String, Node>,
    shape: &PayloadShape,
    executing: Option<String>,
    run: &mut Run,
) {
    if shape.is_empty() {
        return;
    }
    let about = format!("payload of {event}");
    let mut diagnostic =
        Diagnostic::new(CheckCode::Payload, run.id.clone()).declared_by(event.clone());
    if let Some(executing) = executing {
        diagnostic = diagnostic.executing(executing);
    }
    // Accumulated rather than stopped at the first, as every other validation in this workspace is:
    // an event with three fields of the wrong type is three repairs, and reporting one makes the
    // other two look like consequences of it.
    let mut wrong = Vec::new();
    for (path, leaf) in shape.leaves() {
        let reached = reach_into(carried, path);
        let admitted = match &reached {
            Reached::Value(value) => leaf.admits(Some(value)),
            Reached::Absent => leaf.admits(None),
            Reached::Blocked { .. } => false,
        };
        if admitted {
            continue;
        }
        diagnostic = diagnostic.expected(format!("{event}.{path} holds {}", leaf.holds));
        wrong.push(match reached {
            Reached::Value(value) => format!("{path} = {}", quote(value)),
            Reached::Absent => format!("{path} was not carried"),
            Reached::Blocked { at, found } => {
                format!("{at} holds {found}, so {path} is not there to read")
            }
        });
    }
    if wrong.is_empty() {
        run.record(CheckResult::passed(CheckCode::Payload, about));
    } else {
        run.record(CheckResult::failed(
            about,
            diagnostic.observed(wrong.join("; ")),
        ));
    }
}

/// What a dotted leaf path finds in a payload.
enum Reached<'a> {
    /// A value sits there.
    Value(&'a Node),
    /// Nothing was carried under that path, and the walk got as far as it could.
    Absent,
    /// A prefix of the path holds something a field cannot be read out of.
    Blocked {
        /// The prefix.
        at: String,
        /// What it holds instead.
        found: &'static str,
    },
}

/// Walks `path` into a payload, segment by segment.
///
/// A field name cannot contain a dot — the model holds one to `Field::PATTERN` — so splitting on one
/// is a reading of the path rather than a parse that could go wrong.
fn reach_into<'a>(payload: &'a BTreeMap<String, Node>, path: &str) -> Reached<'a> {
    let mut walked = String::new();
    let mut at: Option<&Node> = None;
    for segment in path.split('.') {
        let here = match at {
            None => payload.get(segment),
            Some(Node::Map(fields)) => fields.get(segment),
            Some(value) => {
                return Reached::Blocked {
                    at: walked,
                    found: value.type_name(),
                }
            }
        };
        if !walked.is_empty() {
            walked.push('.');
        }
        walked.push_str(segment);
        match here {
            Some(value) => at = Some(value),
            None => return Reached::Absent,
        }
    }
    at.map_or(Reached::Absent, Reached::Value)
}

/// Requires that the last command published no such event (§10).
///
/// The assertion a wrong implementation passes by accident when nobody writes it down: without it, a
/// suite passes against an implementation that emits everything and refuses nothing.
fn expect_no_event(event: &EventRef, run: &mut Run) -> Flow {
    let Some(executed) = run.last_command.as_ref() else {
        run.record(no_command(&run.id, "the absence of an event"));
        return Flow::Stop;
    };
    let about = format!("no event {event}");
    if executed
        .result
        .direct_events
        .iter()
        .any(|observed| &observed.event == event)
    {
        run.record(CheckResult::failed(
            about,
            Diagnostic::new(CheckCode::NoEvent, run.id.clone())
                .declared_by(event.clone())
                .executing(executed.quoted())
                .expected(format!("event {event} not published"))
                .observed(published(&executed.result)),
        ));
    } else {
        run.record(CheckResult::passed(CheckCode::NoEvent, about));
    }
    Flow::Continue
}

/// Binds the identity the creating branch published, so later steps can name it (§19).
///
/// Failing to bind is an `error` rather than a `failed`: the scenario could not be *arranged*, which
/// is a different verdict from an expectation that did not hold, and every step after it would be
/// asking about an instance that does not exist.
fn capture_instance(
    instance: &InstanceName,
    entity: &EntityRef,
    event: &EventRef,
    field: &str,
    run: &mut Run,
) -> Flow {
    let Some(executed) = run.last_command.as_ref() else {
        run.record(no_command(&run.id, "an instance identity"));
        return Flow::Stop;
    };
    let identity = executed
        .result
        .direct_events
        .iter()
        .find(|observed| &observed.event == event)
        .and_then(|observed| observed.payload.get(field))
        .cloned();
    if let Some(value) = identity {
        run.instances.insert(instance.clone(), value);
        return Flow::Continue;
    }
    run.record(CheckResult::errored(
        format!("the identity `{instance}` of a {entity}"),
        Diagnostic::new(CheckCode::Instance, run.id.clone())
            .declared_by(entity.clone())
            .declared_by(event.clone())
            .executing(executed.quoted())
            .expected(format!("{event}.{field} carries the new identity"))
            .observed(published(&executed.result)),
    ));
    Flow::Stop
}

/// Delivers an already-published event to its bindings a second time (§17).
fn redeliver_event<T: ConformanceTarget>(event: &EventRef, run: &mut Run, target: &T) -> Flow {
    let request = RedeliveryRequest {
        event: event.clone(),
        correlation: run.context.correlation.clone(),
    };
    match target.redeliver_event(request) {
        Ok(()) => Flow::Continue,
        Err(error) => {
            run.record(target_failure(
                &run.id,
                &format!("delivering `{event}` a second time"),
                &error,
            ));
            Flow::Stop
        }
    }
}

/// Requires something of the view last read (§14).
///
/// An expectation that names a different view from the query before it is a **suite** defect, and it
/// is refused rather than silently asserted against whatever happened to be in hand — which would be
/// a mis-assertion nobody could see in a passing run.
fn expect_view(view: &ViewRef, expectation: &ViewExpectation, run: &mut Run) -> Flow {
    // §14's demand could not be made, and the read was therefore not made at all. `failed` rather
    // than `unsupported`, and the distinction is the finding rather than a formality: `unsupported`
    // is for an observation the **target** cannot expose, and a target that cannot expose
    // consistency at all says so by refusing `query_view` — which still lands as `unsupported`, on
    // the path below. Answering a command with no token is a different thing. The specification
    // declares this view `read_your_writes`, which is a claim about the implementation, and §9 gives
    // a command result a token so that claim can be held to something. A result without one has
    // declined the guarantee it declared, which is the implementation contradicting the
    // specification.
    if run
        .unreadable
        .as_ref()
        .is_some_and(|(named, _)| named == view)
    {
        let command = run
            .unreadable
            .as_ref()
            .map_or_else(String::new, |(_, command)| command.clone());
        run.record(CheckResult::failed(
            format!("view {view}"),
            Diagnostic::new(CheckCode::View, run.id.clone())
                .declared_by(view.clone())
                .expected(format!(
                    "`{command}` names the write `{view}` is then read no older than, which is what \
                     `read_your_writes` promises a caller (§14)"
                ))
                .observed(format!(
                    "`{command}` returned no consistency token, so the read was not made: asking at \
                     `Current` instead would answer a weaker question and report it as this one"
                )),
        ));
        return Flow::Continue;
    }
    let required = match Required::of(expectation, run) {
        Ok(required) => required,
        Err(reason) => {
            run.record(unresolvable(view, &run.id, &reason));
            return Flow::Stop;
        }
    };
    let Some((queried, result)) = run.last_view.as_ref() else {
        run.record(CheckResult::errored(
            format!("view {view}"),
            Diagnostic::new(CheckCode::Suite, run.id.clone())
                .declared_by(view.clone())
                .expected(format!("a query of `{view}` precedes an expectation of it"))
                .observed("no view had been read".to_owned()),
        ));
        return Flow::Stop;
    };
    if queried != view {
        run.record(CheckResult::errored(
            format!("view {view}"),
            Diagnostic::new(CheckCode::Suite, run.id.clone())
                .declared_by(view.clone())
                .expected(format!("a query of `{view}` precedes an expectation of it"))
                .observed(format!("the view last read was `{queried}`")),
        ));
        return Flow::Stop;
    }
    let verdict = decide(&required, result);
    let check = view_check(CheckCode::View, &run.id, view, &required, &verdict);
    run.record(check);
    Flow::Continue
}

/// The suite defect of an expectation naming something no earlier step established.
fn unresolvable(view: &ViewRef, id: &ScenarioId, reason: &str) -> CheckResult {
    CheckResult::errored(
        format!("view {view}"),
        Diagnostic::new(CheckCode::Suite, id.clone())
            .declared_by(view.clone())
            .expected(format!(
                "every field `{view}` is matched on refers to something an earlier step established"
            ))
            .observed(reason.to_owned()),
    )
}

// ---- per-scenario state ----------------------------------------------------------------------

/// Whether the scenario continues after a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
    /// Run the next step.
    Continue,
    /// Everything after this step would read state that was never established.
    Stop,
}

/// The command a scenario last executed, and what it answered.
struct Executed {
    command: String,
    input: BTreeMap<String, Node>,
    result: SemanticCommandResult,
}

impl Executed {
    /// The invocation, as §29 quotes one.
    fn quoted(&self) -> String {
        quote_input(&self.command, &self.input)
    }
}

/// What one scenario has established so far.
struct Run {
    id: ScenarioId,
    context: ScenarioContext,
    last_command: Option<Executed>,
    last_view: Option<(ViewRef, SemanticViewResult)>,
    /// The view a read-your-writes read could not be made of, and the command that owes the token.
    unreadable: Option<(ViewRef, String)>,
    instances: BTreeMap<InstanceName, Node>,
    seen: Vec<ObservedEvent>,
    checks: Vec<CheckResult>,
}

impl Run {
    fn new(id: ScenarioId, context: ScenarioContext) -> Self {
        Self {
            id,
            context,
            last_command: None,
            last_view: None,
            unreadable: None,
            instances: BTreeMap::new(),
            seen: Vec::new(),
            checks: Vec::new(),
        }
    }

    fn record(&mut self, check: CheckResult) {
        self.checks.push(check);
    }

    /// Remembers occurrences observed away from a command, without recording one twice.
    fn remember(&mut self, observed: &[ObservedEvent]) {
        for event in observed {
            if !self.seen.contains(event) {
                self.seen.push(event.clone());
            }
        }
    }

    /// Turns a suite's reference into the value this run bound for it.
    fn resolve(&self, value: &ScenarioValue) -> Result<Node, String> {
        match value {
            ScenarioValue::Literal { value } => Ok(value.clone()),
            ScenarioValue::Instance { instance } => self
                .instances
                .get(instance)
                .cloned()
                .ok_or_else(|| format!("no earlier step bound the instance `{instance}`")),
            ScenarioValue::Observed { event, field } => self
                .seen
                .iter()
                .find(|seen| &seen.event == event)
                .ok_or_else(|| format!("`{event}` had not been observed"))
                .and_then(|seen| {
                    seen.payload
                        .get(field)
                        .cloned()
                        .ok_or_else(|| format!("`{event}` carried no field `{field}`"))
                }),
        }
    }

    /// What the run has observed, for a diagnostic that has to say what happened instead.
    fn summary(&self) -> String {
        if self.seen.is_empty() {
            return "no events at all".to_owned();
        }
        self.seen
            .iter()
            .map(|event| event.event.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

// ---- comparison ------------------------------------------------------------------------------

/// `true` when every field `wanted` names is present in `payload` with that value.
///
/// Partial by design (§21): a specification does not determine every value an implementation may
/// legitimately put in a payload, so only the named fields are compared.
fn matches(payload: &BTreeMap<String, Node>, wanted: &BTreeMap<String, Node>) -> bool {
    wanted
        .iter()
        .all(|(field, value)| payload.get(field) == Some(value))
}

/// The first field that does not carry what was required, rendered for a diagnostic.
fn mismatch(
    subject: &str,
    payload: &BTreeMap<String, Node>,
    wanted: &BTreeMap<String, Node>,
) -> Option<String> {
    wanted
        .iter()
        .find_map(|(field, value)| match payload.get(field) {
            Some(observed) if observed == value => None,
            Some(observed) => Some(format!("{subject}.{field} = {}", quote(observed))),
            None => Some(format!("{subject} carried no field `{field}`")),
        })
}

/// What a command published, rendered for a diagnostic.
fn published(result: &SemanticCommandResult) -> String {
    if result.direct_events.is_empty() {
        return "the command published no events".to_owned();
    }
    result
        .direct_events
        .iter()
        .map(|event| {
            if event.payload.is_empty() {
                format!("{} published", event.event)
            } else {
                format!(
                    "{} published with {}",
                    event.event,
                    fields_of(&event.payload)
                )
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// What a view expectation came to.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Verdict {
    /// The view holds what was required.
    Satisfied,
    /// It does not, and what it holds instead.
    Unsatisfied(String),
    /// A predicate could not be decided against a row, which no amount of re-reading changes.
    Undecidable {
        /// The predicate, as written.
        predicate: String,
        /// The row it could not be decided against.
        row: String,
        /// The paths the row does not publish.
        missing: String,
    },
}

/// Decides a view expectation against what a view holds.
///
/// `Unknown` refuses; it does not retry. Evaluating a predicate against a row is three-valued
/// (invariant 5) and only `True` passes: `False` is a failed assertion, and `Unknown` means the row
/// could not answer the question — a defect in the specification or in what the view publishes, and
/// no amount of re-reading changes it. So it is reported where it is met, even inside an
/// [`EventuallyView`](ScenarioStep::EventuallyView), where the temptation is to poll to the deadline
/// and report a timeout: the same defect in a slower, less legible costume.
fn decide(required: &Required, result: &SemanticViewResult) -> Verdict {
    match required {
        Required::Contains(fields) => {
            if result.rows.iter().any(|row| matches(row, fields)) {
                Verdict::Satisfied
            } else {
                Verdict::Unsatisfied(rows(result))
            }
        }
        Required::Excludes(fields) => {
            if result.rows.iter().any(|row| matches(row, fields)) {
                Verdict::Unsatisfied(rows(result))
            } else {
                Verdict::Satisfied
            }
        }
        Required::Satisfies(predicate) => satisfies(predicate, result),
    }
}

/// A view expectation with every reference the suite carried resolved to what this run bound.
///
/// The suite says "the row whose `invoice_id` is the invoice step one created"; only the run knows
/// which invoice that is. Resolving once, before the query in an
/// [`EventuallyView`](ScenarioStep::EventuallyView) is retried, also means a reference nothing bound
/// is a **suite** defect reported once rather than the same complaint on every ask.
enum Required {
    /// A row matching these values is present.
    Contains(BTreeMap<String, Node>),
    /// No row matching these values is present.
    Excludes(BTreeMap<String, Node>),
    /// Every row satisfies this, and there is at least one.
    Satisfies(Predicate),
}

impl Required {
    /// Resolves every reference an expectation carries against what this run has bound.
    fn of(expectation: &ViewExpectation, run: &Run) -> Result<Self, String> {
        let resolve = |fields: &BTreeMap<String, ScenarioValue>| {
            fields
                .iter()
                .map(|(field, value)| {
                    run.resolve(value)
                        .map(|node| (field.clone(), node))
                        .map_err(|reason| format!("`{field}`: {reason}"))
                })
                .collect::<Result<BTreeMap<String, Node>, String>>()
        };
        Ok(match expectation {
            ViewExpectation::Contains { fields } => Self::Contains(resolve(fields)?),
            ViewExpectation::Excludes { fields } => Self::Excludes(resolve(fields)?),
            ViewExpectation::Satisfies { predicate } => Self::Satisfies(predicate.clone()),
        })
    }
}

/// Every row satisfies the predicate, and there is at least one.
///
/// "At least one" is part of the assertion, and it is the difference between a check and a
/// formality: every row of an empty view satisfies everything, so a target that publishes nothing
/// would pass an invariant check that made no such demand.
fn satisfies(predicate: &Predicate, result: &SemanticViewResult) -> Verdict {
    if result.rows.is_empty() {
        return Verdict::Unsatisfied("the view holds no rows".to_owned());
    }
    for row in &result.rows {
        let facts = row_facts(row);
        let outcome = predicate.outcome(&facts);
        match outcome.truth {
            Truth::True => {}
            Truth::False => {
                return Verdict::Unsatisfied(format!(
                    "{} in {}",
                    outcome.expression,
                    quote_row(row)
                ))
            }
            Truth::Unknown => {
                let missing = outcome
                    .missing_facts()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                return Verdict::Undecidable {
                    predicate: predicate.to_string(),
                    row: quote_row(row),
                    missing: if missing.is_empty() {
                        "the comparison could not be made".to_owned()
                    } else {
                        missing
                    },
                };
            }
        }
    }
    Verdict::Satisfied
}

/// Turns a verdict into the check it records.
fn view_check(
    code: CheckCode,
    id: &ScenarioId,
    view: &ViewRef,
    required: &Required,
    verdict: &Verdict,
) -> CheckResult {
    let about = format!("view {view}");
    let diagnostic = Diagnostic::new(code, id.clone()).declared_by(view.clone());
    match verdict {
        Verdict::Satisfied => CheckResult::passed(code, about),
        Verdict::Unsatisfied(observed) => CheckResult::failed(
            about,
            diagnostic
                .expected(wanted(view, required))
                .observed(observed.clone()),
        ),
        Verdict::Undecidable {
            predicate,
            row,
            missing,
        } => CheckResult::errored(
            about,
            Diagnostic::new(CheckCode::Predicate, id.clone())
                .declared_by(view.clone())
                .expected(format!("`{view}` publishes every path `{predicate}` reads"))
                .observed(format!("{row} binds nothing for {missing}")),
        ),
    }
}

/// What a view expectation requires, in one line, with its references resolved.
fn wanted(view: &ViewRef, required: &Required) -> String {
    match required {
        Required::Contains(fields) if fields.is_empty() => {
            format!("{view} holds a row")
        }
        Required::Contains(fields) => {
            format!("{view} holds a row where {}", fields_of(fields))
        }
        Required::Excludes(fields) if fields.is_empty() => {
            format!("{view} holds no rows")
        }
        Required::Excludes(fields) => {
            format!("{view} holds no row where {}", fields_of(fields))
        }
        Required::Satisfies(predicate) => {
            format!("every row of {view} satisfies `{predicate}`, and it holds at least one")
        }
    }
}

/// `field = value, …`, for a required or observed row.
fn fields_of(fields: &BTreeMap<String, Node>) -> String {
    fields
        .iter()
        .map(|(field, value)| format!("{field} = {}", quote(value)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// What a view holds, rendered for a diagnostic.
fn rows(result: &SemanticViewResult) -> String {
    if result.rows.is_empty() {
        return "the view holds no rows".to_owned();
    }
    result
        .rows
        .iter()
        .map(quote_row)
        .collect::<Vec<_>>()
        .join("; ")
}

/// One row, rendered for a diagnostic.
fn quote_row(row: &ViewRow) -> String {
    format!("{{{}}}", fields_of(row))
}

/// Projects a view row into the facts a predicate reads.
///
/// The untyped counterpart of [`flatten`](crate::flatten), and untyped on purpose: a runner holds a
/// suite and a target and never an `EssIr` (§21, §22), so there are no declared types here to walk
/// beside the value. What a view publishes is a flat list of named projected fields, and a nested
/// value is walked structurally — `total` holding `{amount, currency}` binds `total.amount` and
/// `total.currency`.
///
/// A sequence binds nothing, for the reason [`flatten`](crate::flatten) does not project a list: a
/// fact path has no index, so `lines.0.quantity` is not a path this model can spell. A row that does
/// not publish what a predicate reads makes that predicate `Unknown`, which
/// [`decide`] reports rather than retries.
fn row_facts(row: &ViewRow) -> FactStore {
    let mut facts = FactStore::new();
    for (field, value) in row {
        if let Ok(path) = FactPath::new(field) {
            bind(&path, value, &mut facts);
        }
    }
    facts
}

/// Binds one scalar leaf, or walks into a mapping.
fn bind(path: &FactPath, value: &Node, facts: &mut FactStore) {
    match value {
        Node::Null | Node::Seq(_) => {}
        Node::Bool(flag) => facts.set(path.clone(), FactValue::bool(*flag)),
        Node::Number(number) => facts.set(path.clone(), FactValue::from(*number)),
        Node::Text(text) => facts.set(path.clone(), FactValue::text(text.clone())),
        Node::Map(entries) => {
            for (key, entry) in entries {
                bind(&path.child(key), entry, facts);
            }
        }
    }
}

// ---- shared diagnostics ----------------------------------------------------------------------

/// The check a target failure records, as §28 classifies it.
fn target_failure(id: &ScenarioId, operation: &str, error: &TargetError) -> CheckResult {
    let diagnostic = Diagnostic::new(CheckCode::Target, id.clone())
        .expected(format!("the target can carry out {operation}"))
        .observed(error.to_string());
    if error.is_unsupported() {
        CheckResult::unsupported(operation.to_owned(), diagnostic)
    } else {
        CheckResult::errored(operation.to_owned(), diagnostic)
    }
}

/// An assertion about a command, in a scenario where none has run.
///
/// A suite defect rather than an implementation defect: every assertion after an
/// [`ExecuteCommand`](ScenarioStep::ExecuteCommand) is about *that* invocation, so one with no
/// invocation before it is asking about nothing.
fn no_command(id: &ScenarioId, subject: &str) -> CheckResult {
    CheckResult::errored(
        format!("{subject} of the last command"),
        Diagnostic::new(CheckCode::Suite, id.clone())
            .expected(format!(
                "a command is executed before {subject} is required of it"
            ))
            .observed("no command had been executed in this scenario".to_owned()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::{SuiteFormat, SuiteProvenance};
    use aep_domain::evidence::SpecDigest;
    use aep_domain::facts::FactSource;

    fn view() -> ViewRef {
        "billing.invoice.InvoiceById".parse().expect("a view name")
    }

    fn scenario() -> ScenarioId {
        ScenarioId::parse("billing.invoice.CreateInvoice/outcome/accepted").expect("an id")
    }

    fn money(amount: f64) -> Node {
        let mut fields = BTreeMap::new();
        fields.insert(
            "amount".to_owned(),
            Node::Number(aep_domain::facts::Number::new(amount).expect("finite")),
        );
        Node::Map(fields)
    }

    fn row(amount: f64) -> ViewRow {
        let mut fields = ViewRow::new();
        fields.insert("total".to_owned(), money(amount));
        fields
    }

    #[test]
    fn the_runners_clock_advances_on_every_read_so_a_deadline_can_bound_anything() {
        // The property the whole no-sleep story rests on: a clock that stood still would make an
        // `Eventually*` step ask its target forever, which is why this is a trait of its own rather
        // than `aep_engine::clock::Clock`.
        let mut clock = AdvancingClock::new(1_000, 250);
        assert_eq!(clock.now().epoch_millis(), 1_000);
        assert_eq!(clock.now().epoch_millis(), 1_250);
        assert_eq!(clock.now().epoch_millis(), 1_500);

        let mut stalled = AdvancingClock::new(0, 0);
        assert_eq!(
            stalled.now().epoch_millis(),
            0,
            "a step of zero is raised to one rather than accepted"
        );
        assert_eq!(stalled.now().epoch_millis(), 1);
    }

    #[test]
    fn ids_come_from_the_suite_and_from_nothing_ambient() {
        let mut ids = Ids::for_suite(&ConformanceSuite::new(SuiteProvenance {
            suite_version: SuiteFormat::CURRENT,
            system: "billing".to_owned(),
            specification_version: "v3".to_owned(),
            spec_digest: SpecDigest::new("0123456789abcdef").expect("a digest"),
            compiler_version: "0.1.0".to_owned(),
            generator_version: "0.1.0".to_owned(),
            synthesizer_version: "0.1.0".to_owned(),
        }));

        assert_eq!(
            ids.correlation().as_str(),
            "billing-0123456789abcdef-000001"
        );
        assert_eq!(
            ids.correlation().as_str(),
            "billing-0123456789abcdef-000002"
        );

        // A system name is an ESS name and a correlation id is an AEP identifier; the two charsets
        // are not the same rule, so the seed is reduced rather than assumed.
        let mut awkward = Ids::seeded("Billing / v3 — main");
        assert_eq!(awkward.correlation().as_str(), "Billing-v3-main-000001");
        let mut empty = Ids::seeded("///");
        assert_eq!(empty.correlation().as_str(), "ess-000001");
    }

    #[test]
    fn a_view_that_holds_nothing_does_not_satisfy_an_invariant_by_being_empty() {
        // "At least one row" is part of the assertion: every row of an empty view satisfies
        // everything, so a target that published nothing would otherwise pass an invariant check.
        let predicate = Predicate::parse_expression("total.amount >= 0").expect("a predicate");
        let expectation = Required::Satisfies(predicate);

        assert_eq!(
            decide(&expectation, &SemanticViewResult::default()),
            Verdict::Unsatisfied("the view holds no rows".to_owned())
        );
        assert_eq!(
            decide(&expectation, &SemanticViewResult::of([row(1.0)])),
            Verdict::Satisfied
        );
        assert!(matches!(
            decide(&expectation, &SemanticViewResult::of([row(-1.0)])),
            Verdict::Unsatisfied(_)
        ));
    }

    #[test]
    fn a_predicate_a_row_cannot_answer_is_reported_rather_than_retried() {
        // Invariant 5 from the runner's side: `Unknown` is not `False`. A row that does not publish
        // what the predicate reads is a defect in the specification or in the view, and no amount of
        // re-reading changes it — so it must not become a timeout, which is the same defect wearing
        // a slower costume.
        let predicate = Predicate::parse_expression("settlement_window >= 0").expect("a predicate");
        let expectation = Required::Satisfies(predicate);

        let verdict = decide(&expectation, &SemanticViewResult::of([row(1.0)]));

        let Verdict::Undecidable { missing, .. } = &verdict else {
            panic!(
                "a row that binds nothing for the path must be undecidable, not false: {verdict:?}"
            )
        };
        assert!(
            missing.contains("settlement_window"),
            "the report names the path the row does not publish: {missing}"
        );

        let check = view_check(
            CheckCode::EventualView,
            &scenario(),
            &view(),
            &expectation,
            &verdict,
        );
        assert_eq!(
            check.status,
            Status::Error,
            "nobody could execute the check, which is not the same as the implementation being wrong"
        );
        assert_eq!(check.code, CheckCode::Predicate);
    }

    #[test]
    fn a_nested_row_binds_the_paths_a_predicate_spells() {
        // The untyped counterpart of `flatten`: a view publishes named fields, and a nested value is
        // walked structurally, because the runner holds no `EssIr` to walk types beside it.
        let mut fields = ViewRow::new();
        fields.insert("total".to_owned(), money(3.0));
        fields.insert("invoice_id".to_owned(), Node::Text("abc".to_owned()));
        fields.insert("lines".to_owned(), Node::Seq(vec![money(1.0)]));

        let facts = row_facts(&fields);

        assert_eq!(
            facts.fact(&FactPath::new("total.amount").expect("a path")),
            Some(FactValue::number(3.0).expect("finite"))
        );
        assert_eq!(
            facts.fact(&FactPath::new("invoice_id").expect("a path")),
            Some(FactValue::text("abc"))
        );
        assert_eq!(
            facts.fact(&FactPath::new("lines").expect("a path")),
            None,
            "a fact path has no index, so no path can name an element of a list"
        );
    }

    #[test]
    fn an_empty_field_set_means_a_row_exists_and_not_that_anything_will_do() {
        let contains = Required::Contains(BTreeMap::new());
        let excludes = Required::Excludes(BTreeMap::new());

        assert_eq!(
            decide(&contains, &SemanticViewResult::of([row(1.0)])),
            Verdict::Satisfied
        );
        assert!(matches!(
            decide(&contains, &SemanticViewResult::default()),
            Verdict::Unsatisfied(_)
        ));
        assert_eq!(
            decide(&excludes, &SemanticViewResult::default()),
            Verdict::Satisfied
        );
        assert!(matches!(
            decide(&excludes, &SemanticViewResult::of([row(1.0)])),
            Verdict::Unsatisfied(_)
        ));
    }
}
