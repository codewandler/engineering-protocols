# Passkeys, planned in the repository

The same work as [`examples/development-passkeys`](../development-passkeys), planned the other way
round. This one keeps its plan **here**, as markdown under `.engineering/planning/`:

```text
.engineering/
├── project.yaml
└── planning/
    ├── initiative/passwordless-authentication.md
    ├── epic/passkey-sign-in.md
    ├── story/passkey-registration.md
    ├── story/passkey-login.md
    ├── story/passkey-recovery.md
    ├── task/webauthn-ceremony.md
    └── task/assertion-verification.md
```

One artifact per file, YAML frontmatter the tooling reads, and a markdown body it never interprets.

## Try it

```console
protocol artifact list     --store .engineering/planning --root ../..
protocol artifact board    --store .engineering/planning --root ../..
protocol artifact graph    --store .engineering/planning --root ../.. | dot -Tsvg > plan.svg
protocol artifact validate --store .engineering/planning --root ../..
```

`--root` points at the document tree the lifecycles come from — this repository. Inside a project
whose `project.yaml` names its own protocol tree, neither flag is needed: `--store` defaults to
`<project>/.engineering/planning`.

The entity surface reads this store as readily as it reads a manifest:

```console
protocol entity list --planning .engineering/planning
```

## The contrast with `development-passkeys` is the point

`examples/development-passkeys` keeps the *same* stories in Linear and points at them from
`artifacts.yaml`:

```yaml
- id: story:AUTH-141
  kind: story
  location:
    provider: linear
    reference: AUTH-141
```

Neither example is the recommended one. They are the two arrangements the protocol supports, and
they are both here so that neither reads as an accident:

| | `development-passkeys` | `planning-passkeys` |
|---|---|---|
| where the plan lives | Linear | this repository |
| how AEP sees it | `artifacts.yaml`, `location: {provider, reference}` | `.engineering/planning/*.md`, `location: <path>` |
| what moves a status | Linear's UI | `protocol artifact move`, refused against the kind's lifecycle |
| history | Linear's | `git log` |
| what it costs | AEP cannot check the plan's own contents | the plan is one more thing in the repository to review |

The graph is the same shape either way, which is what
[`ArtifactLocation`](../../crates/aep-domain/src/artifact.rs) exists to make true: location is
metadata, and only the graph is normative. A team can start in one arrangement and move to the
other without the protocol noticing.

## What this fixture is used by

`crates/protocol-cli/tests/planning_cli.rs` drives the real binary against it: that the store
validates clean, that `list --format json` is byte-identical across two runs, and that
`protocol entity list --planning` counts what is here.
