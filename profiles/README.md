# Profiles

A profile bundles a protocol version, a workflow, a set of principles, a capability policy and a
completion condition, so a task selects one line instead of enumerating three dozen rules.

`development.fast`, `development.standard` and `development.critical` are three points on one scale:
the latter two extend the former, and extension can only make completion harder.

`development.driven` sits beside that scale rather than on it. It extends `development.standard`
with exactly one capability — `command.execute` — and is for runs under `protocol drive`, where the
plugin's `driven-surface` hook holds a model's shell to `protocol artifact …` and `protocol trace
…`. The grant exists because the planning store's whole vocabulary is CLI verbs and a driven step
that cannot run one cannot create the artifact its transition is guarded on. Do not choose it for
interactive work, and do not choose it under a harness that cannot constrain a shell to a named
surface: the profile is the outer bound and the hook is the inner one, and without the hook there is
only the outer bound. The reasoning, and what it does not claim, is in the document's own header.
