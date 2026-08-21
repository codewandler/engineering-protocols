# Planning in a governed artifact store

Copy this into your project's `AGENTS.md`, or append it to the one you have. Codex reads
`AGENTS.md` from the repository root down to the working directory and concatenates what it finds,
root first, so this text is in context for every turn without anything being invoked. That is the
whole reason it exists beside the skill: **a rule that only holds once a skill is opened does not
hold in the turn where the model decides not to open it.**

It is named `AGENTS.planning.md` here rather than `AGENTS.md` so that it is inert in this
repository — this tree has its own working agreement, and a second instruction file three
directories down would be concatenated onto it for anyone working on the integration.

The full model, the worked decomposition and the on-disk format are in
[`skills/planning/SKILL.md`](./skills/planning/SKILL.md). Only the guardrails and the discovery
rule are repeated here, because only those have to survive a session that never reads the skill.

---

## Planning artifacts

Plan items are markdown files under `.engineering/planning/<kind>/<slug>.md` — YAML frontmatter the
`protocol` CLI owns, and a body you and the operator own.

### Ask the CLI; do not recite the vocabulary

Which kinds exist, which statuses each holds and which moves are legal come from validated lifecycle
documents. This file names none of them, on purpose: a prose copy of a validated document is not
validated and goes stale the first time a kind gains a status. Reading the answer costs one command
and cannot be wrong.

| Question | Command |
|---|---|
| What kinds can I create? | `protocol artifact kinds` |
| What edges exist between artifacts? | `protocol artifact relations` |
| What statuses does this kind have, and what moves where? | `protocol artifact lifecycle <kind>` |
| What is already in the store? | `protocol artifact list [--kind k] [--status s] [--format json]` |
| What does it look like as a board? | `protocol artifact board [--kind k]` |
| How is it wired together? | `protocol artifact graph` |

Before the first write of a session, run `protocol artifact list` and `protocol artifact kinds`.

### Four guardrails

1. **A status changes only through `protocol artifact move`.** Never edit the `status:` field, and
   never write it into a file with a patch or a heredoc. The CLI validates the move against the
   kind's lifecycle; a hand-edited status is an unvalidated one, indistinguishable in the file from
   a legal one.
2. **The body is edited directly — and only the body.** Patch below the closing `---` and touch no
   line above it. There is no CLI verb for prose and there should not be one; equally, a whole-file
   rewrite re-types machine-owned frontmatter by hand, and a faithful copy is indistinguishable from
   a silently-altered one until something downstream breaks.
3. **After a batch of edits, run `protocol artifact validate` and relay its output verbatim.** It
   accumulates every problem rather than stopping at the first and exits 1 if any remain. "Validation
   failed" throws away the only part the operator can act on.
4. **A refusal is the answer, not an obstacle.** A refused move exits 1 and names every status legal
   from where the artifact stands. Relay that list. Do not retry with a different spelling, do not
   route around it by editing the file, and do not walk an artifact through an intermediate status
   without saying so.

### Who decides

New artifacts and body edits need no confirmation beyond the request that prompted them — a draft is
cheap and reversible. A status move is a claim about the state of the world and is the operator's to
make: propose it, name the ids, and wait. A move the operator already asked for by name is already
confirmed; asking again is noise. Never perform a bulk move autonomously.

### Prerequisite

`protocol` must be on `PATH`. Without the CLI none of the above is executable, and hand-writing the
frontmatter it owns is the failure these rules exist to prevent — say so rather than improvising a
store.
