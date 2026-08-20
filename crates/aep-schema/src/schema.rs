//! JSON Schema generation.
//!
//! The generated schemas are the interoperability contract: anything that can produce or consume
//! AEP documents can validate them without linking this crate. They are derived from the same
//! Rust types the engine executes, so a schema cannot describe a document the engine would
//! reject on structure — the two cannot drift.

use aep_domain::action::ActionRequest;
use aep_domain::artifact::ArtifactLifecycle;
use aep_domain::event::EventEnvelope;
use aep_domain::evidence::Evidence;
use aep_domain::raw::{
    RawArtifactManifest, RawPrinciple, RawProfile, RawProtocol, RawTask, RawWorkflow,
};
use schemars::schema::RootSchema;
use schemars::{schema_for, JsonSchema};

use crate::format::canonical_json;

/// One generated schema.
#[derive(Debug, Clone)]
pub struct GeneratedSchema {
    /// The schema's title, taken from the Rust type name.
    pub name: &'static str,
    /// The file it is written to, relative to `schemas/generated/`.
    pub filename: String,
    /// What this schema describes, for the index.
    pub describes: &'static str,
    /// The schema itself.
    pub schema: RootSchema,
}

impl GeneratedSchema {
    /// The schema as canonical JSON, ending in a newline.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        canonical_json(&self.schema)
    }
}

/// Builds one entry.
fn entry<T: JsonSchema>(
    name: &'static str,
    slug: &str,
    describes: &'static str,
) -> GeneratedSchema {
    GeneratedSchema {
        name,
        filename: format!("{slug}.schema.json"),
        describes,
        schema: schema_for!(T),
    }
}

/// Every schema this build publishes.
///
/// The six document schemas are what a project's files are validated against; the three
/// interchange schemas are what a harness exchanges with the engine at run time.
pub fn generated_schemas() -> Vec<GeneratedSchema> {
    vec![
        entry::<RawProtocol>("RawProtocol", "protocol", "a protocol declaration"),
        entry::<RawPrinciple>("RawPrinciple", "principle", "a principle"),
        entry::<RawWorkflow>("RawWorkflow", "workflow", "a workflow state machine"),
        entry::<RawProfile>("RawProfile", "profile", "a profile"),
        entry::<RawTask>("RawTask", "task", "a task"),
        entry::<aep_domain::project::RawProjectConfig>(
            "RawProjectConfig",
            "project",
            "what an adopting project says about itself",
        ),
        entry::<RawArtifactManifest>(
            "RawArtifactManifest",
            "artifact-manifest",
            "a project's artifact manifest",
        ),
        entry::<ArtifactLifecycle>(
            "ArtifactLifecycle",
            "artifact-lifecycle",
            "the statuses one artifact kind may hold",
        ),
        entry::<Evidence>("Evidence", "evidence", "one piece of submitted evidence"),
        entry::<ActionRequest>(
            "ActionRequest",
            "action-request",
            "an action put to the engine",
        ),
        entry::<EventEnvelope>("EventEnvelope", "event", "one audit event"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_a_schema_for_every_document_and_interchange_type() {
        let schemas = generated_schemas();
        assert_eq!(schemas.len(), 11);

        let filenames: Vec<&str> = schemas
            .iter()
            .map(|entry| entry.filename.as_str())
            .collect();
        assert!(filenames.contains(&"workflow.schema.json"), "{filenames:?}");
        assert!(
            filenames.contains(&"artifact-manifest.schema.json"),
            "{filenames:?}"
        );
    }

    #[test]
    fn schemas_are_valid_json_with_a_title_and_a_trailing_newline() {
        for entry in generated_schemas() {
            let json = entry.to_json().expect("serialises");
            assert!(
                json.ends_with('\n'),
                "{} lacks a trailing newline",
                entry.name
            );
            let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
            assert_eq!(
                parsed["title"].as_str(),
                Some(entry.name),
                "{} has the wrong title",
                entry.name
            );
        }
    }

    #[test]
    fn generation_is_deterministic() {
        let first = generated_schemas()
            .iter()
            .map(|entry| entry.to_json().expect("serialises"))
            .collect::<Vec<_>>();
        let second = generated_schemas()
            .iter()
            .map(|entry| entry.to_json().expect("serialises"))
            .collect::<Vec<_>>();
        assert_eq!(
            first, second,
            "schema generation must be byte-identical between runs"
        );
    }

    #[test]
    fn the_workflow_schema_publishes_identifier_patterns() {
        let workflow = generated_schemas()
            .into_iter()
            .find(|entry| entry.name == "RawWorkflow")
            .expect("the workflow schema is generated");
        let json = serde_json::to_value(&workflow.schema).expect("serialises");
        let definitions = json["definitions"].as_object().expect("has definitions");
        let state_id = definitions
            .get("StateId")
            .expect("StateId is referenced by the workflow schema");
        assert!(
            state_id["pattern"].as_str().is_some(),
            "the published schema must carry the identifier pattern, not just `type: string`"
        );
    }
}
