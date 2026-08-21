---
format: aep.planning-md/1
id: story:driver-spec-crate
kind: story
status: active
title: 'aep-driver-spec: the step map, validated before anything runs'
summary: A leaf crate over aep-domain holding RawStepMap, StepMap, PinnedWorkflowRef, the cursor types, ToolConfig and both cross-validation phases.
owner: driver
tags:
- driver
relations:
- decomposes: epic:reference-driver
revision: 3
---
# Story: `aep-driver-spec` — the step map, validated before anything runs

## Outcome

An author writes a step map and finds out at load time that it is wrong, instead of finding out
mid-run when a model call has already been paid for and half a workflow has already happened.

## Context

The map is the document that says what happens in each state, and it pins the workflow it belongs to.
A workflow major bump must orphan a map pinned to the old one, loudly, at load. The crate is a leaf
on `aep-domain` only — the same shape `aep-backend-markdown` already has — because everything it
holds is a document type and a validation, and none of it touches the world.

## Acceptance

- A map whose `workflow` pin names a major the registry no longer has is refused at load, naming the
  pin and what is available.
- Validation runs in **two phases**: states and named verifiers at load; evidence kinds and the
  workflow pin at run start, because the protocol in force comes from the task, which no document
  loader has seen. `Verifier::ExternalTool` is exempt at load.
- `PinnedWorkflowRef` refuses a reference with no major version, and the schema it publishes makes
  the version group **required** — an editor cannot tell an author a map is fine that the loader will
  refuse.
- The manifest carries `[lints] workspace = true`, and the crate's row is added to invariant 9's list
  in `AGENTS.md` in the same change.

## Out of Scope

Executing anything. This crate has no executor, no process spawn and no file write outside its own
tests.

## Open Questions

None blocking. Whether cursor types belong here or beside the run directory is settled here: they are
data the router reads, and the router is pure.
