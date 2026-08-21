# The surface `run-agents.sh` must expose, so its own acceptance is checkable

Written **before** the runner exists, in the `establish_verifiers` state of run `W4-1/1`. Everything
here is a contract the checks in `../` depend on: change a name in this file and a check goes red,
which is the point — a verifier that discovers the interface by reading the implementation is a
verifier that agrees with whatever the implementation happens to do.

Nothing here widens the specification. It fixes the *handles* the specification leaves to the
implementer, in the two places the decomposed tasks explicitly demand one:

* `task:agent-eval-scratch-fixture` — "the fixture build is reachable without running a stage (the
  handle is the implementer's choice: a flag, a sourceable function, a separate script)". This file
  makes that choice, because a check cannot be written against an unnamed choice.
* `task:agent-eval-decomposer-stage` / `task:agent-eval-reviewer-stage` — "checked against a saved
  store state, not by paying for three more live runs". Replay needs an entry point; here it is.

## Invocations

| Invocation | Must do | May call the API |
|---|---|---|
| `run-agents.sh` | build the fixture, run both stages live, print one verdict table | yes |
| `run-agents.sh --offline` | re-check both trace documents against `../fixtures/` | **no** |
| `run-agents.sh --build-fixture-only` | build the fixture and the baseline, then stop | **no** |
| `run-agents.sh --baseline <dir>` | print the baseline record for the store in `<dir>` to stdout | **no** |

`--build-fixture-only` prints exactly these three lines to stdout, each once, each an absolute path:

```
scratch: /…/agent-eval.XXXXXX
fixture: /…/agent-eval.XXXXXX/project
baseline: /…/agent-eval.XXXXXX/baseline.tsv
```

`scratch` is the directory that survives the run and holds everything; `fixture` is the git
repository both stages work in; `baseline` is the record described in
[`baseline-record.md`](./baseline-record.md).

## Environment

Two the specification names, and the checks read their defaults from the script rather than from
memory (`M4`):

| Variable | Meaning |
|---|---|
| `EVAL_MODEL` | the model both stages run |
| `EVAL_MAX_TURNS` | the per-stage turn bound |

Seven the checks need, each existing because a decomposed task asks for a fact that cannot be
observed any other way. All of them are inert when unset, so a live run is unaffected by their
existence.

| Variable | Effect the runner must honour | Which acceptance row needs it |
|---|---|---|
| `EVAL_FIXTURE_SRC=<dir>` | build the fixture from `<dir>` instead of `examples/planning-passkeys` | `F7` |
| `EVAL_FIXTURE_DIRTY_PROBE=1` | write one file into the fixture immediately **before** the R3 clean-tree assertion | `F4` |
| `EVAL_REPLAY_STORE_DECOMPOSER=<dir>` | take stage 1's post-state store from `<dir>`; run no session | `S3`–`S5` |
| `EVAL_REPLAY_STORE_REVIEWER=<dir>` | take stage 2's post-state store from `<dir>`; run no session | `V5` |
| `EVAL_REPLAY_TRANSCRIPT_DECOMPOSER=<file>` | take stage 1's transcript from `<file>`; run no session | `R3`–`R6` |
| `EVAL_REPLAY_TRANSCRIPT_REVIEWER=<file>` | take stage 2's transcript from `<file>`; run no session | `V6`, `R3`–`R6` |
| `EVAL_PRINT_COMMAND=1` | print each `claude -p` invocation it would issue, then exit 0 without issuing it | `R8` |
| `EVAL_SPEC_DECOMPOSER=<file>` | check stage 1 against `<file>` instead of the shipped trace document | `R4` |
| `EVAL_SPEC_REVIEWER=<file>` | the same, for stage 2 | `R4` |

Rules that hold for every one of them:

* **A replay never calls the API.** With both `EVAL_REPLAY_STORE_*` and both
  `EVAL_REPLAY_TRANSCRIPT_*` set, a whole `run-agents.sh` is hermetic — which is what makes `R2`,
  `R3`, `R4` and `R5` demonstrable without money.
* **A replay changes where a fact comes from, never whether it is asserted.** Every row still
  prints, still carries a verdict, and still gates. A replay that turned rows off would make the
  discrimination checks assert nothing, which is the failure mode `A vacuous check is a failed
  check` names.
* **The word `skip` appears in no verdict** (`O8`), replaying or not.

## Output the checks parse

The verdict table is one row per line. A check reads a row by its **id**, so the ids in
[`verdict-rows.txt`](./verdict-rows.txt) are load-bearing text, not decoration:

```
PASS  D3   every created artifact is in the story lifecycle's initial status
FAIL  P1   git status --porcelain in the scratch project is empty
```

Two shapes are accepted for the leading verdict word, so the runner may follow either sibling:
`PASS`/`FAIL`/`note` as `run.sh` writes them, or `ok`/`gap`/`unk` as `protocol trace check` writes
them. The id must be the second field either way.

The last two lines of a run are the cost and the scratch path (`R7`):

```
cost: $0.42
inspect the run yourself: /…/agent-eval.XXXXXX
```
