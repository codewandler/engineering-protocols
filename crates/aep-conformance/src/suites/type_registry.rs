//! A type can be described instead of hard-coded.
//!
//! §47 is what stops a generic harness from being a switch statement over the types this repository
//! happened to know about. Asked "what is this thing, which commands may target it, which relations
//! may it have, is it immutable?", a backend answers from data — so an organisation can add an
//! entity type nobody here ever heard of and the tooling keeps working. §78.16 makes that promise
//! checkable. The `mutable` flag is the part with teeth: a harness that cannot discover that a
//! review result may not be edited will happily offer an edit, and the refusal then arrives as a
//! surprise at the point of use rather than as a fact about the type.

use aep_contract::command::CommandEnvelope;
use aep_contract::registry::TypeDescriptor;
use aep_contract::testing::block_on;
use aep_domain::command::{Command, CreateEntity};
use aep_domain::entity::{EntityLocator, EntityRef, EntityType};
use aep_domain::ids::{CommandId, IdempotencyKey};
use aep_domain::node::Node;

use crate::harness::{Backend, Harness, ORGANISATION, SPACE};
use crate::report::SuiteReport;

/// Runs the type-registry suite.
pub fn run<B: Backend>(backend: &B) -> SuiteReport {
    let harness = Harness::new(SUITE);
    let mut report = SuiteReport::new("type-registry");

    let describable = "a type an entity was created with can be described";
    let commands = "a descriptor names at least one command that may target the type";
    let immutable = "an immutable type reports that it may not be changed";
    let mutable = "a type that may be changed is not reported as immutable";
    let unknown = "an unknown type is reported as not found rather than as an empty descriptor";

    // Ask about a type the backend has actually accepted an entity of, rather than about a constant
    // written here: discovering semantics from the store is the position §47 puts a harness in.
    let design = match observe(&harness, backend, "design") {
        Ok(entity_type) => entity_type,
        Err(detail) => {
            report.aborted(describable, detail);
            return report;
        }
    };

    let descriptor = match block_on(backend.describe_type(&design)) {
        Ok(descriptor) => {
            report.expect(
                describable,
                descriptor.entity_type == design,
                format!(
                    "asking about `{design}` returned a descriptor for `{}`",
                    descriptor.entity_type
                ),
            );
            Some(descriptor)
        }
        Err(error) => {
            report.expect(
                describable,
                false,
                format!("`{design}` was created a moment ago and cannot be described: {error}"),
            );
            None
        }
    };

    report.expect(
        commands,
        descriptor
            .as_ref()
            .is_some_and(|descriptor| !descriptor.commands.is_empty()),
        format!(
            "`{design}` is described by {}, so a harness cannot tell what it is allowed to do with \
             one",
            descriptor.as_ref().map_or_else(
                || "nothing at all".to_owned(),
                |descriptor| format!("{} command(s)", descriptor.commands.len())
            )
        ),
    );

    report.expect(
        mutable,
        descriptor
            .as_ref()
            .is_some_and(|descriptor| descriptor.mutable),
        format!("`{design}` reports {}", mutability(descriptor.as_ref())),
    );

    match observe(&harness, backend, "review-result") {
        Ok(review) => {
            let described = block_on(backend.describe_type(&review)).ok();
            report.expect(
                immutable,
                described
                    .as_ref()
                    .is_some_and(|descriptor| !descriptor.mutable),
                format!(
                    "`{review}` reports {} — a record that can be edited after the fact is not \
                     evidence of what anybody concluded",
                    mutability(described.as_ref())
                ),
            );
        }
        Err(detail) => report.aborted(immutable, detail),
    }

    match EntityType::parse("acme.widget/v1") {
        Ok(nonsense) => match block_on(backend.describe_type(&nonsense)) {
            Ok(descriptor) => report.expect(
                unknown,
                false,
                format!(
                    "nothing declares `{nonsense}` and yet it was described as {:?}",
                    descriptor.summary
                ),
            ),
            Err(error) => report.expect(
                unknown,
                error.code() == "not_found",
                format!(
                    "describing an undeclared type failed with `{}` rather than `not_found`",
                    error.code()
                ),
            ),
        },
        Err(error) => report.aborted(unknown, error.to_string()),
    }

    report
}

/// How a descriptor answers "may this be changed?", for a failure detail worth reading.
fn mutability(descriptor: Option<&TypeDescriptor>) -> String {
    descriptor.map_or_else(
        || "nothing, because it could not be described at all".to_owned(),
        |descriptor| format!("mutable: {}", descriptor.mutable),
    )
}

/// Creates an entity of `aep.<kind>/v1` and reports the type the backend says it has.
fn observe<B: Backend>(harness: &Harness, backend: &B, kind: &str) -> Result<EntityType, String> {
    let entity_type = EntityType::parse(&format!("aep.{kind}/v1")).map_err(|e| e.to_string())?;
    let locator = address(kind, "sample")?;
    harness
        .execute(
            backend,
            envelope(
                harness,
                kind,
                Command::CreateEntity(CreateEntity {
                    entity_type,
                    locator: locator.clone(),
                    data: Node::Map(
                        [
                            (
                                "title".to_owned(),
                                Node::from("An entity of a discoverable type"),
                            ),
                            ("status".to_owned(), Node::from("active")),
                        ]
                        .into(),
                    ),
                }),
            )?,
        )
        .map_err(|error| format!("an entity of `aep.{kind}/v1` could not be created: {error}"))?;
    let id = block_on(backend.resolve(&locator)).map_err(|error| error.to_string())?;
    Ok(harness
        .read(backend, &EntityRef::new(id))?
        .metadata
        .entity_type)
}

/// The name this suite mints into every identifier it creates.
///
/// A full run drives all sixteen suites against one backend, and the harness numbers its generated
/// identifiers from zero for each of them. Two suites using them raw issue the same idempotency key
/// for different commands and create entities at the same address; both are refused, and the failure
/// reads as a fault in the backend rather than as a collision between suites.
const SUITE: &str = "type-registry";

/// An address no other suite in the same run uses.
fn address(kind: &str, tag: &str) -> Result<EntityLocator, String> {
    EntityLocator::new(ORGANISATION, SPACE, kind, format!("{kind}-{SUITE}-{tag}"))
        .map_err(|error| error.to_string())
}

/// An envelope whose command id and idempotency key no other suite in the same run uses.
fn envelope(
    harness: &Harness,
    tag: &str,
    payload: Command,
) -> Result<CommandEnvelope<Command>, String> {
    let command_id = CommandId::new(format!("cmd-{SUITE}-{tag}")).map_err(|e| e.to_string())?;
    let key = IdempotencyKey::new(format!("key-{SUITE}-{tag}")).map_err(|e| e.to_string())?;
    Ok(harness.envelope(command_id, payload, harness.context_with_key(&key)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use aep_backend_memory::MemoryBackend;

    #[test]
    fn the_reference_backend_can_describe_what_it_stores() {
        let report = run(&MemoryBackend::new());
        assert!(report.passed(), "{report}");
    }

    #[test]
    fn a_backend_that_describes_nothing_fails() {
        let backend = crate::faulty::FaultyBackend::new(
            MemoryBackend::new(),
            crate::faulty::Fault::UndescribeTypes,
        );
        let report = run(&backend);
        assert!(!report.passed(), "{report}");
    }
}
