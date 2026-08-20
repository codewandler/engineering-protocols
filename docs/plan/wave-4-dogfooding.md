# Wave 4 — run it on ourselves

> **In progress.**

Goal: **this repository governed by its own protocol.** Every claim the project makes is currently
proven by tests we wrote. Nothing has been governed by it, which is the one gap no amount of test
coverage closes.

The fastest honest test is to point it at the work in front of us. If the protocol cannot describe
how this repository is actually built — waves, a design doc per wave, a green gate, a tag — then it
cannot describe anyone else's work either, and we would rather find that out here.

## W4.1 A project can be discovered

Today the CLI must be told where everything is: `--task`, `--artifacts`, `--root`. An adopting team
therefore types four paths before they see anything. Design §20 and §72 describe the convention that
fixes it:

```text
project/
├── docs/                     human-readable artifacts
└── .engineering/
    ├── project.yaml          which protocol, which profile, where the tree is
    ├── artifacts.yaml        what exists and how it relates
    ├── task.yaml             what is being worked on
    └── state.yaml            where the execution got to
```

`.engineering/` may also hold `principles/` and `profiles/` of the project's own, merged over the
protocol tree — because no organisation's rules are entirely somebody else's.

Deliverable: `aep_domain::project::ProjectConfig`, `aep_engine::project::{discover, load}`, CLI
commands that work with no arguments inside a project.

## W4.2 `.engineering/` for this repository

A profile describing how this project is actually built, not how a generic project is: a design
document before a wave, a plan before implementation, the gate green before a commit, a tag per wave,
a changelog entry for anything user-visible. Those are the rules we already follow by hand.

The interesting output is what it **refuses**. A rule we cannot express, or one that fires when it
should not, is a finding about the protocol rather than about the repository.

## W4.3 What it says about the last three waves

Run the resolved plan against the evidence that actually exists — the tags, the CI runs, the design
documents — and publish the result. Including the parts that fail.

## Out of scope

A durable backend, federated artifact graphs, triggered work. Those are wave 5 candidates.
