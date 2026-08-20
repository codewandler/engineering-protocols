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
use ess_domain::spec::RawSpecFile;
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
///
/// Every schema goes through [`crate::alias`] on the way out, because `schemars` cannot see
/// `#[serde(alias = "…")]`: a derived schema describes the canonical spelling only, and with
/// `deny_unknown_fields` beside it that turns the other accepted spelling into an editor error on a
/// document the parser reads without complaint.
fn entry<T: JsonSchema>(
    name: &'static str,
    slug: &str,
    describes: &'static str,
) -> GeneratedSchema {
    let mut schema = schema_for!(T);
    crate::alias::publish(&mut schema);
    GeneratedSchema {
        name,
        filename: format!("{slug}.schema.json"),
        describes,
        schema,
    }
}

/// Every schema this build publishes.
///
/// The document schemas are what a project's files are validated against; the interchange schemas
/// are what a harness exchanges with the engine at run time. The specification schema is the one an
/// editor loads: an author writing an ESS gets the field names checked as they type, rather than
/// after a build that runs somewhere else.
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
        entry::<RawSpecFile>(
            "RawSpecFile",
            "ess",
            "one file of an executable system specification",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_a_schema_for_every_document_and_interchange_type() {
        let schemas = generated_schemas();
        let filenames: Vec<&str> = schemas
            .iter()
            .map(|entry| entry.filename.as_str())
            .collect();

        // Named rather than counted: a count tells whoever broke it that a number changed, and a
        // list tells them which contract went missing.
        for expected in [
            "protocol.schema.json",
            "principle.schema.json",
            "workflow.schema.json",
            "profile.schema.json",
            "task.schema.json",
            "project.schema.json",
            "artifact-manifest.schema.json",
            "artifact-lifecycle.schema.json",
            "evidence.schema.json",
            "action-request.schema.json",
            "event.schema.json",
            "ess.schema.json",
        ] {
            assert!(filenames.contains(&expected), "{expected} is not published");
        }
    }

    #[test]
    fn no_two_schemas_claim_the_same_file() {
        // Two entries with one filename means the second silently overwrites the first, and the
        // committed tree looks complete while missing a contract.
        let mut seen = std::collections::BTreeSet::new();
        for entry in generated_schemas() {
            assert!(
                seen.insert(entry.filename.clone()),
                "{} is published twice",
                entry.filename
            );
        }
    }

    #[test]
    fn every_schema_says_what_it_describes() {
        // `describes` is what the generated index shows a reader; an empty one publishes a row
        // that tells them nothing.
        for entry in generated_schemas() {
            assert!(
                !entry.describes.is_empty(),
                "{} describes nothing",
                entry.name
            );
        }
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

    /// A validator for one definition of a generated schema.
    ///
    /// The published document schemas are checked against the documents this repository ships;
    /// these check the forms it happens not to ship, which is where a schema drifts away from the
    /// parser unnoticed.
    fn definition(filename: &str, name: &str) -> jsonschema::Validator {
        let entry = generated_schemas()
            .into_iter()
            .find(|entry| entry.filename == filename)
            .unwrap_or_else(|| panic!("{filename} is published"));
        let mut json: serde_json::Value =
            serde_json::from_str(&entry.to_json().expect("serialises")).expect("is JSON");
        let definitions = json["definitions"].take();
        assert!(
            definitions[name].is_object(),
            "{filename} does not define {name}"
        );
        let root = serde_json::json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "allOf": [{ "$ref": format!("#/definitions/{name}") }],
            "definitions": definitions,
        });
        jsonschema::validator_for(&root).expect("a usable schema")
    }

    /// Asserts a document fragment validates.
    fn accepts(validator: &jsonschema::Validator, instance: &serde_json::Value, why: &str) {
        let problems: Vec<String> = validator
            .iter_errors(instance)
            .map(|error| format!("{}: {error}", error.instance_path()))
            .collect();
        assert!(problems.is_empty(), "{why}: {}", problems.join("; "));
    }

    /// Asserts a document fragment is refused.
    fn refuses(validator: &jsonschema::Validator, instance: &serde_json::Value, why: &str) {
        assert!(!validator.is_valid(instance), "{why}");
    }

    #[test]
    fn the_evidence_requirement_schema_accepts_a_bare_kind_and_refuses_a_mapping_with_no_kind() {
        let validator = definition("principle.schema.json", "EvidenceRequirement");
        accepts(
            &validator,
            &serde_json::json!("verification"),
            "`- verification` is what every principle here writes",
        );
        accepts(
            &validator,
            &serde_json::json!({ "kind": "test_result", "independent": true }),
            "the mapping form supplies `at_least` itself",
        );
        refuses(
            &validator,
            &serde_json::json!({ "independent": true }),
            "`from_node` refuses a mapping with no `kind`",
        );
        refuses(
            &validator,
            &serde_json::json!("test_results"),
            "`EvidenceKind::parse` refuses a kind it does not know",
        );
    }

    #[test]
    fn the_evidence_kind_schema_publishes_the_spellings_the_parser_accepts() {
        let validator = definition("principle.schema.json", "EvidenceKind");
        accepts(
            &validator,
            &serde_json::json!("source_diff"),
            "`source_diff` is an accepted spelling of `diff`",
        );
        refuses(
            &validator,
            &serde_json::json!("diff_source"),
            "no parser accepts an invented spelling",
        );
    }

    #[test]
    fn the_artifact_requirement_schema_accepts_a_bare_kind_and_refuses_an_unknown_status() {
        let validator = definition("principle.schema.json", "ArtifactRequirement");
        accepts(
            &validator,
            &serde_json::json!("design"),
            "an artifact kind on its own is the shorthand form",
        );
        accepts(
            &validator,
            &serde_json::json!({ "kind": "design", "status": "in-review" }),
            "`parse_status` reads `in-review` as `in_review`",
        );
        refuses(
            &validator,
            &serde_json::json!({ "status": "approved" }),
            "`from_node` refuses a mapping with no `kind`",
        );
        refuses(
            &validator,
            &serde_json::json!({ "kind": "design", "status": "nearly" }),
            "`parse_status` refuses a status no lifecycle has",
        );
    }

    #[test]
    fn the_relation_requirement_schema_accepts_a_bare_relation_and_refuses_an_invented_one() {
        let validator = definition("principle.schema.json", "RelationRequirement");
        accepts(
            &validator,
            &serde_json::json!("designs"),
            "`relation: designs` is the shorthand form",
        );
        refuses(
            &validator,
            &serde_json::json!("inspires"),
            "`RelationKind::parse` refuses a relation the graph has no meaning for",
        );
    }

    #[test]
    fn the_review_requirement_schema_accepts_a_bare_kind_and_refuses_a_mapping_naming_no_subject() {
        let validator = definition("principle.schema.json", "ReviewRequirement");
        accepts(
            &validator,
            &serde_json::json!("design"),
            "an artifact kind on its own means an approving review of one",
        );
        accepts(
            &validator,
            &serde_json::json!({ "kind": "design", "human": true }),
            "`kind` is read as `subject_kind`",
        );
        refuses(
            &validator,
            &serde_json::json!({ "human": true }),
            "`from_node` refuses a review requirement that says neither what nor which",
        );
        refuses(
            &validator,
            &serde_json::json!({ "subject_kind": "design", "result": "maybe" }),
            "`from_node` refuses a disposition no review can have",
        );
    }

    #[test]
    fn the_approval_requirement_schema_accepts_a_bare_id_and_refuses_a_mapping_naming_none() {
        let validator = definition("principle.schema.json", "ApprovalRequirement");
        accepts(
            &validator,
            &serde_json::json!("security-review"),
            "`approvals: [security-review]` is the shorthand form",
        );
        refuses(
            &validator,
            &serde_json::json!({ "human": true }),
            "`from_node` refuses a mapping with neither `approval` nor `id`",
        );
    }

    #[test]
    fn the_conditional_requirement_schema_accepts_both_spellings_of_what_is_then_owed() {
        let validator = definition("principle.schema.json", "ConditionalRequirement");
        for spelling in ["require", "requires"] {
            accepts(
                &validator,
                &serde_json::json!({
                    "when": { "change.architectural": true },
                    spelling: { "artifacts": ["architecture-design"] },
                }),
                "`parse_conditional` reads both spellings",
            );
        }
        refuses(
            &validator,
            &serde_json::json!({ "when": { "change.architectural": true } }),
            "`parse_conditional` refuses a condition that requires nothing",
        );
    }

    #[test]
    fn the_requirement_set_schema_accepts_every_form_a_state_may_be_written_in() {
        let validator = definition("principle.schema.json", "RequirementSet");
        for form in [
            serde_json::json!("tests.unit.failed == 0"),
            serde_json::json!(["tests.unit.failed == 0", "design.approved"]),
            serde_json::json!({ "evidence": ["test_result"], "approvals": "security-review" }),
            serde_json::json!({ "change.architectural": true }),
            serde_json::Value::Null,
        ] {
            accepts(
                &validator,
                &form,
                "`RequirementSet::from_node` reads this form",
            );
        }
        refuses(
            &validator,
            &serde_json::json!(3),
            "`RequirementSet::from_node` refuses a number",
        );
    }

    #[test]
    fn the_objective_schema_accepts_the_one_line_form_every_task_is_written_with() {
        let validator = definition("task.schema.json", "Objective");
        accepts(
            &validator,
            &serde_json::json!("add-passkey-support"),
            "a task states its objective on one line",
        );
        refuses(
            &validator,
            &serde_json::json!({ "details": "at length" }),
            "`Objective`'s `Deserialize` refuses a mapping with no summary",
        );
        refuses(
            &validator,
            &serde_json::json!(7),
            "`Objective`'s `Deserialize` refuses anything but a string or a mapping",
        );
    }

    #[test]
    fn the_capability_policy_schema_accepts_the_spelling_every_principle_uses() {
        let validator = definition("principle.schema.json", "CapabilityPolicy");
        accepts(
            &validator,
            &serde_json::json!({ "require_approval": ["production.write"] }),
            "`require_approval` is what the shipped principles write",
        );
        refuses(
            &validator,
            &serde_json::json!({ "requires_approval": ["production.write"] }),
            "`deny_unknown_fields` refuses a misspelt key",
        );
        refuses(
            &validator,
            &serde_json::json!({
                "approval_required": ["production.write"],
                "require_approval": ["deployment.create"],
            }),
            "serde reads both spellings into one field, so giving both is a duplicate",
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
