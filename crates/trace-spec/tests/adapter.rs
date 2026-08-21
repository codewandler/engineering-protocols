//! The adapter against two real transcripts, committed byte for byte.
//!
//! Both were produced by the plugin eval on 2026-08-21 against Claude Code `2.1.238`, and both are
//! in the repository rather than generated, because an adapter over a format that is not a stable
//! public schema (design D1) is only as good as the runs it has actually met. The numbers asserted
//! here are the ones the design's § 2 tables state and the eval's own `jq` metrics block computes;
//! where the two definitions differ, the deviation is named in the test that carries it.
//!
//! The pair exists to cover a difference: `7hTYjT` wrote its three story bodies with `Edit`, and
//! `1huAQG` wrote them with `Write`. Same task, same prompt, two tool surfaces — which is exactly
//! the kind of variation an adapter that had only ever seen one run would encode as a rule.

use serde_json::Value;
use trace_domain::ir::TraceIr;
use trace_spec::adapter::{read_transcript, CLAUDE_CODE_STREAM_JSON};

/// The run the design's § 2.6 step table was measured on: 36 lines, 11 steps, three `Edit`s.
const SEVEN_H: &[u8] = include_bytes!("fixtures/plugin-eval-7hTYjT.jsonl");

/// The sibling run: 37 lines, three `Write`s where the other used `Edit`.
const ONE_HU: &[u8] = include_bytes!("fixtures/plugin-eval-1huAQG.jsonl");

/// Reads a fixture, insisting it read.
fn read(bytes: &[u8]) -> TraceIr {
    read_transcript(bytes).expect("a committed transcript reads")
}

/// The per-family census as a sorted list, which is what an assertion can print readably.
fn families(ir: &TraceIr) -> Vec<(String, usize)> {
    ir.census().events_by_family.into_iter().collect()
}

/// Per-tool `(calls, errors, results, input_bytes, result_bytes)`.
fn traffic(ir: &TraceIr) -> Vec<(String, usize, usize, usize, usize, usize)> {
    ir.tool_traffic()
        .into_iter()
        .map(|(tool, t)| {
            (
                tool,
                t.calls,
                t.errors,
                t.results,
                t.input_bytes,
                t.result_bytes,
            )
        })
        .collect()
}

#[test]
fn the_census_of_the_edit_run_is_thirty_six_events_and_nothing_the_adapter_could_not_read() {
    let ir = read(SEVEN_H);
    let census = ir.census();

    assert_eq!(census.events, 36, "one IR event per line, on this run");
    assert_eq!(
        census.opaque_events, 0,
        "an opaque event is not a defect, but a run of 36 lines with none is the claim that this \
         adapter understands the whole of the observed format"
    );
    assert_eq!(
        families(&ir),
        vec![
            ("assistant_text".to_owned(), 6),
            ("assistant_thinking".to_owned(), 2),
            ("rate_limit".to_owned(), 1),
            ("run_outcome".to_owned(), 1),
            ("session_start".to_owned(), 1),
            ("synthetic_injection".to_owned(), 1),
            ("thinking_estimate".to_owned(), 2),
            ("tool_call".to_owned(), 11),
            ("tool_result".to_owned(), 11),
        ]
    );
    assert_eq!(
        ir.assistant_event_count(),
        19,
        "19 streamed assistant events — an artefact of streaming, not a cost measure"
    );
    assert_eq!(
        ir.api_request_count(),
        8,
        "and 8 actual API requests: several streamed events share one request id"
    );
    assert_eq!(
        census.repeated_call_groups, 0,
        "no byte-identical call was made twice"
    );
    assert_eq!(ir.adapter, CLAUDE_CODE_STREAM_JSON);
}

#[test]
fn the_edit_runs_tool_traffic_is_eleven_calls_and_not_one_failure() {
    let ir = read(SEVEN_H);
    assert_eq!(
        traffic(&ir),
        vec![
            ("Bash".to_owned(), 4, 0, 4, 1_116, 2_739),
            ("Edit".to_owned(), 3, 0, 3, 6_863, 637),
            ("Read".to_owned(), 3, 0, 3, 361, 4_656),
            ("Skill".to_owned(), 1, 0, 1, 423, 47),
        ],
        "calls, errors, results, input bytes, result bytes"
    );

    let calls: usize = ir.tool_traffic().values().map(|t| t.calls).sum();
    let errors: usize = ir.tool_traffic().values().map(|t| t.errors).sum();
    assert_eq!((calls, errors), (11, 0));

    // The asymmetry design § 2.5 is built on, visible in one line of this run: `Read` sent 361
    // bytes of arguments and took 4 656 bytes of context back, `Edit` sent 6 863 and took 637.
    // Input is model output at output prices; result bytes are injected into the next request.
    let traffic = ir.tool_traffic();
    assert!(traffic["Read"].result_bytes > traffic["Read"].input_bytes * 10);
    assert!(traffic["Edit"].input_bytes > traffic["Edit"].result_bytes * 10);
}

#[test]
fn the_first_four_events_carry_no_timestamp_and_the_step_table_is_the_designs() {
    let ir = read(SEVEN_H);

    // The state the "derive nothing from a timestamp that was not recorded" rule is load-bearing
    // in, asserted before the durations that depend on it: the `init`, the rate-limit event and
    // the first two thinking estimates carry no `timestamp` at all. A `gen` measured across them
    // would be a subtraction against zero, and a reader would read it as a fast first turn.
    for event in ir.events.iter().take(4) {
        assert_eq!(
            event.timestamp, None,
            "line {} records no timestamp",
            event.source_line
        );
        assert_eq!(event.timestamp_ms, None);
    }
    assert!(
        ir.events[4].timestamp_ms.is_some(),
        "and the fifth does, so the fixture reaches both sides of the rule"
    );

    let steps: Vec<(String, Option<i64>, Option<i64>)> = ir
        .steps()
        .into_iter()
        .map(|step| (step.tool, step.gen_ms, step.exec_ms))
        .collect();
    assert_eq!(
        steps,
        vec![
            ("Skill".to_owned(), Some(1_486), Some(35)),
            ("Bash".to_owned(), Some(1_290), Some(187)),
            ("Bash".to_owned(), Some(1_088), Some(21)),
            ("Bash".to_owned(), Some(3_205), Some(38)),
            ("Read".to_owned(), Some(555), Some(36)),
            ("Read".to_owned(), Some(560), Some(6)),
            ("Read".to_owned(), Some(305), Some(16)),
            ("Edit".to_owned(), Some(8_742), Some(26)),
            ("Edit".to_owned(), Some(5_968), Some(28)),
            ("Edit".to_owned(), Some(4_482), Some(9)),
            ("Bash".to_owned(), Some(80), Some(13)),
        ],
        "the design's § 2.6 table, derived from recorded timestamps and measured by nothing"
    );

    let census = ir.census();
    assert_eq!(census.inference_total_ms, Some(27_761));
    assert_eq!(census.tool_exec_total_ms, Some(415));
    // 27 761 ms of inference against 415 ms of tool execution: the wall clock is 98.5% model.
    assert!(census.inference_total_ms > census.tool_exec_total_ms.map(|ms| ms * 60));
}

#[test]
fn the_opening_record_says_which_model_which_permissions_and_which_plugin() {
    let ir = read(SEVEN_H);
    let start = ir.session_start().expect("the run has an opening record");

    assert_eq!(start.model.as_deref(), Some("claude-sonnet-5"));
    assert_eq!(start.permission_mode.as_deref(), Some("dontAsk"));
    assert_eq!(
        start.api_key_source.as_deref(),
        Some("none"),
        "the logged-in session paid: an exported key here once billed an account with no credits"
    );
    assert_eq!(start.harness_version.as_deref(), Some("2.1.238"));
    assert_eq!(
        start.output_style.as_deref(),
        Some("default"),
        "not the operator's own style — the run's config home was isolated"
    );
    assert!(start.cwd.is_some());

    let plugins = start.plugins.as_deref().expect("plugins were recorded");
    assert_eq!(
        plugins.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
        vec!["engineering-protocols"],
        "exactly one plugin, which is what hermeticity looks like in the first event"
    );
    assert_eq!(plugins[0].version.as_deref(), Some("0.1.0"));
    assert_eq!(
        plugins[0].source.as_deref(),
        Some("engineering-protocols@inline")
    );
    assert!(plugins[0].path.is_some());

    assert_eq!(start.tools.as_ref().map(Vec::len), Some(32));
    assert_eq!(start.slash_commands.as_ref().map(Vec::len), Some(47));
    assert!(start
        .skills
        .as_ref()
        .expect("skills were recorded")
        .iter()
        .any(|skill| skill == "engineering-protocols:planning"));
    assert!(start
        .agents
        .as_ref()
        .expect("agents were recorded")
        .iter()
        .any(|agent| agent == "engineering-protocols:decomposer"));
}

#[test]
fn the_skill_call_and_the_result_it_was_answered_with_are_one_pair() {
    let ir = read(SEVEN_H);
    let (_, call) = ir
        .tool_calls()
        .into_iter()
        .find(|(_, call)| call.name == "Skill")
        .expect("the run invoked a skill");

    // A skill invocation is a tool call: there is no distinct event kind for it (design § 2.2),
    // and this is the claim the eval's grep was reaching for and could not state.
    assert_eq!(
        call.argument("skill").and_then(Value::as_str),
        Some("engineering-protocols:planning")
    );

    let (_, result) = ir
        .result_of(call)
        .expect("the call was correlated to its result by TraceIr::new, not by the adapter");
    assert_eq!(
        result.field("commandName").and_then(Value::as_str),
        Some("engineering-protocols:planning")
    );
    assert_eq!(
        result.field("success").and_then(Value::as_bool),
        Some(true),
        "the skill ran to completion, structurally — not because its output looked right"
    );
    assert_eq!(result.is_error, Some(false));
}

#[test]
fn the_terminal_record_is_the_source_of_every_resource_fact() {
    let ir = read(SEVEN_H);
    let outcome = ir.run_outcome().expect("the run has a terminal record");

    assert_eq!(outcome.is_error, Some(false));
    assert_eq!(outcome.subtype.as_deref(), Some("success"));
    assert_eq!(outcome.terminal_reason.as_deref(), Some("completed"));
    assert_eq!(outcome.stop_reason.as_deref(), Some("end_turn"));
    assert_eq!(
        outcome.api_error_status, None,
        "recorded as `null`, and `null` is not a status"
    );
    assert_eq!(
        outcome.permission_denials,
        Some(0),
        "an empty list is the harness saying `none`"
    );
    assert_eq!(outcome.subagents_spawned, Some(0));
    assert_eq!(outcome.num_turns, Some(13));
    assert_eq!(outcome.ttft_ms, Some(1_915), "read, never derived");
    assert_eq!(outcome.duration_ms, Some(42_167));
    assert_eq!(outcome.duration_api_ms, Some(42_955));
    assert_eq!(outcome.time_to_request_ms, Some(50));
    let cost = outcome.total_cost_usd.expect("the run recorded a cost");
    assert!((cost - 0.273_658_9).abs() < 1e-6, "the run cost {cost} USD");

    let usage = outcome.usage.as_ref().expect("the run recorded usage");
    assert_eq!(usage.input_tokens, Some(16));
    assert_eq!(usage.output_tokens, Some(3_824));
    assert_eq!(usage.cache_read_input_tokens, Some(313_513));
    assert_eq!(usage.cache_creation_input_tokens, Some(20_168));
    assert_eq!(
        usage.thinking_tokens,
        Some(34),
        "the billed figure, not the mid-stream estimate of 80"
    );
    assert_eq!(
        usage.iterations,
        Some(1),
        "an array's length, and nothing like the other three quantities"
    );
    assert_eq!(usage.speed.as_deref(), Some("standard"));
    assert_eq!(usage.service_tier.as_deref(), Some("standard"));

    let by_model = outcome
        .model_usage
        .as_ref()
        .expect("the run recorded a per-model breakdown");
    assert_eq!(
        by_model.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["claude-haiku-4-5-20251001", "claude-sonnet-5"],
        "two models: the one that did the work and the one that summarised for it"
    );
    assert_eq!(by_model["claude-sonnet-5"].output_tokens, Some(3_824));
    assert!(by_model["claude-sonnet-5"].cost_usd > by_model["claude-haiku-4-5-20251001"].cost_usd);

    // The last mid-stream estimate is a different quantity from the billed one above, and the two
    // must not be conflated: 80 estimated, 34 billed.
    assert_eq!(
        ir.last_thinking_estimate().map(|(_, tokens)| tokens),
        Some(80)
    );
}

#[test]
fn the_rate_limit_state_at_the_start_of_the_run_is_a_fact_about_money() {
    let ir = read(SEVEN_H);
    let (_, state) = ir
        .rate_limit()
        .expect("the run recorded a rate-limit event");
    assert_eq!(state.status.as_deref(), Some("allowed_warning"));
    assert_eq!(state.limit_type.as_deref(), Some("seven_day"));
    assert_eq!(state.utilization, Some(0.64));
    assert_eq!(
        state.is_using_overage,
        Some(false),
        "this run was not paid for out of overage, which no other part of the record says"
    );
    assert_eq!(state.resets_at, Some(1_787_796_000));
}

#[test]
fn the_skill_put_its_own_text_into_the_conversation() {
    let ir = read(SEVEN_H);
    let injected: Vec<&str> = ir
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            trace_domain::ir::EventKind::SyntheticInjection { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(injected.len(), 1, "one synthetic injection in this run");
    assert!(
        injected[0].contains("Base directory for this skill:"),
        "the skill's own content entering the model's context — a stronger fact than `available` \
         or `invoked` (design § 2.8), recorded and given no expectation kind"
    );
}

#[test]
fn the_digest_names_the_bytes_and_reading_them_twice_gives_the_same_ir() {
    let first = read(SEVEN_H);
    let second = read(SEVEN_H);

    assert_eq!(first.transcript_digest.len(), 64);
    assert!(
        first
            .transcript_digest
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
        "64 lowercase hex characters, so anyone holding the file can recompute it with sha256sum"
    );
    assert_ne!(first.transcript_digest, read(ONE_HU).transcript_digest);

    // Invariant 9, over the whole IR and not just its digest.
    let once = serde_json::to_vec(&first).expect("the IR serializes");
    let twice = serde_json::to_vec(&second).expect("the IR serializes");
    assert_eq!(once, twice, "same bytes in, byte-identical IR out");
}

#[test]
fn the_write_run_has_the_same_shape_with_a_different_tool_surface() {
    let ir = read(ONE_HU);
    let census = ir.census();

    assert_eq!(census.events, 37);
    assert_eq!(census.opaque_events, 0);
    assert_eq!(
        families(&ir),
        vec![
            ("assistant_text".to_owned(), 5),
            ("assistant_thinking".to_owned(), 2),
            ("rate_limit".to_owned(), 1),
            ("run_outcome".to_owned(), 1),
            ("session_start".to_owned(), 1),
            ("synthetic_injection".to_owned(), 1),
            ("thinking_estimate".to_owned(), 4),
            ("tool_call".to_owned(), 11),
            ("tool_result".to_owned(), 11),
        ]
    );
    assert_eq!(ir.assistant_event_count(), 18);
    assert_eq!(ir.api_request_count(), 8);

    // The difference the two fixtures exist to cover: this run composed the three story bodies
    // with `Write` where `7hTYjT` used `Edit`. Same task, same prompt, a different tool surface —
    // and an adapter that had only ever met one of them would have encoded the other as a rule.
    assert_eq!(
        traffic(&ir),
        vec![
            ("Bash".to_owned(), 4, 0, 4, 1_186, 1_706),
            ("Read".to_owned(), 3, 0, 3, 375, 4_682),
            ("Skill".to_owned(), 1, 0, 1, 300, 47),
            ("Write".to_owned(), 3, 0, 3, 4_244, 651),
        ]
    );
    assert!(
        !ir.tool_traffic().contains_key("Edit"),
        "and no Edit at all, which is the point of keeping both"
    );

    let start = ir.session_start().expect("an opening record");
    assert_eq!(
        start
            .plugins
            .as_deref()
            .expect("plugins were recorded")
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        vec!["engineering-protocols"]
    );
    assert_eq!(start.api_key_source.as_deref(), Some("none"));

    let outcome = ir.run_outcome().expect("a terminal record");
    assert_eq!(outcome.is_error, Some(false));
    assert_eq!(outcome.permission_denials, Some(0));
}
