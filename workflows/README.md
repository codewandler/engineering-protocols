# Workflows

State machines work moves through, one document each. States declare *phases*; principles time their
obligations against phases, which is what lets one principle apply to a development workflow, a
release workflow and an incident workflow without being rewritten.

Validated by `RawWorkflow` → `Workflow`, which rejects unreachable states, dead ends, transitions to
states that do not exist, and rollback declared on an irreversible state.
