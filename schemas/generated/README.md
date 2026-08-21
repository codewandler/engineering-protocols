# Generated schemas

**Do not edit these files.** They are generated from the Rust types by `cargo xtask schema`, and CI
fails if they differ from what the types produce.

They are the interoperability contract: anything that produces or consumes these documents can
validate them without linking the Rust crates.

| file | Rust type | describes |
| --- | --- | --- |
| [`protocol.schema.json`](protocol.schema.json) | `RawProtocol` | a protocol declaration |
| [`principle.schema.json`](principle.schema.json) | `RawPrinciple` | a principle |
| [`workflow.schema.json`](workflow.schema.json) | `RawWorkflow` | a workflow state machine |
| [`profile.schema.json`](profile.schema.json) | `RawProfile` | a profile |
| [`task.schema.json`](task.schema.json) | `RawTask` | a task |
| [`project.schema.json`](project.schema.json) | `RawProjectConfig` | what an adopting project says about itself |
| [`artifact-manifest.schema.json`](artifact-manifest.schema.json) | `RawArtifactManifest` | a project's artifact manifest |
| [`artifact-lifecycle.schema.json`](artifact-lifecycle.schema.json) | `ArtifactLifecycle` | the statuses one artifact kind may hold |
| [`evidence.schema.json`](evidence.schema.json) | `Evidence` | one piece of submitted evidence |
| [`action-request.schema.json`](action-request.schema.json) | `ActionRequest` | an action put to the engine |
| [`event.schema.json`](event.schema.json) | `EventEnvelope` | one audit event |
| [`ess.schema.json`](ess.schema.json) | `RawSpecFile` | one file of an executable system specification |
| [`planning-document.schema.json`](planning-document.schema.json) | `RawPlanningFrontmatter` | the frontmatter of one markdown planning document |
