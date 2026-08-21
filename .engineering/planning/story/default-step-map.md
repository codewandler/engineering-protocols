---
format: aep.planning-md/1
id: story:default-step-map
kind: story
status: proposed
title: The first step map, and the tree row that loads it
summary: drivers/development/default.yaml over adp/default/1, its generated JSON Schema, and drivers/ as the last row of the document tree.
owner: driver
tags:
- driver
relations:
- decomposes: epic:reference-driver
- depends_on: story:driver-spec-crate
revision: 2
---
# Story: The first step map, and the tree row that loads it

## Outcome

A reader can open one YAML file and see what the default development workflow actually does at each
state — which step runs, what it is allowed to touch, and what would make it move on.

## Context

Until this exists the driver has decisions and no document to walk. The map goes under `drivers/`,
which has been a reserved directory name with nothing writing to it since wave 2, and `drivers/` is
added as the **last** row of the document tree loader so that no existing tree's load order moves.
The generated schema is what makes an author's editor agree with the loader.

## Acceptance

- `drivers/development/default.yaml` loads, pins `adp/default/1`, and covers every state that
  workflow declares — a state with no step is a refusal, not a silent skip.
- `schemas/generated/driver-steps.schema.json` is generated from the type and checked by the ordinary
  generate-check, so a drifted schema fails the gate.
- `drivers/` is the last entry of the loader's tree table, and a repository with no `drivers/`
  directory still loads exactly as before.
- The map's every named verifier resolves at load; its evidence kinds are checked at run start.

## Out of Scope

A second map, a second profile's map, and anything under `incidents/`, `migrations/` or `releases/`.
One workflow, walked properly, is what proves the shape.

## Open Questions

Whether a step map may extend another the way a profile extends a profile. Decides: driver owner.
Not blocking — one map cannot demonstrate the need, which is the argument for not designing it yet.
