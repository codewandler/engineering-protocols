# Input transcripts for the trace-document checks

Hand-written in the `establish_verifiers` state of run `W4-1/1`, before either trace document
existed. They are **inputs to a check**, not evidence about anything: nothing here came from a
model, and no claim in this repository rests on one of these files describing a real run.

They are not `eval/fixtures/`. That directory holds the transcripts of the live run 1, committed by
`task:agent-eval-live-evidence`, and it is what `--offline` replays. These are the small, ugly,
deliberately-broken transcripts a discrimination check needs — `T3`, `T4`, `T5` and `V6` each need a
transcript that violates exactly one bound, and a real run does not helpfully produce one.

| File | What it is |
|---|---|
| `decomposer-clean.jsonl` | a decomposer stage that held every bound in R12 |
| `decomposer-ran-a-move.jsonl` | the same, plus one `protocol artifact move` — `T3` |
| `plan-reviewer-clean.jsonl` | a reviewer stage that held every bound in R12 |
| `plan-reviewer-created-an-artifact.jsonl` | the same, plus one `protocol artifact new` — `T4` |
| `no-tool-calls.jsonl` | a run that reached the model and called nothing — the positive control, `T5` |
| `plan-reviewer-empty-final-text.jsonl` | a reviewer whose last turn said nothing — `V6`/`P5` |
| `plan-reviewer-no-read-verb.jsonl` | a reviewer that never ran a `protocol artifact` read — `V6`/`P6` |
| `plan-reviewer-no-subagent.jsonl` | a reviewer session that spawned nothing — `V6`/`P7` |

Each is one `system/init` event, some assistant and user events, and one terminal `result` event, in
the `stream-json` shape `run.sh` already records. Only the fields the `trace-spec/1` adapter reads
are populated; the numbers are plausible rather than measured, and no bound in this repository is
calibrated against them.
