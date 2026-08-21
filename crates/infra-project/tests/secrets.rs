//! The one thing a generated patch must never contain.
//!
//! The hard rule of the infrastructure family, third enforcement. The scanner sanitizes before it
//! writes; `infra-domain` refuses an unsanitized bundle (`INFRA-SECRET-001`) so a secret value
//! cannot enter the IR even through a bundle the scanner never touched; and this asserts the
//! consequence at the far end — that nothing which *did* enter the IR reaches a file this crate
//! writes.
//!
//! It is a byte scan on purpose. A structural argument ("the projection only reads gaps, and gaps
//! carry no secret values") is exactly the kind of reasoning that stays true until somebody adds a
//! field to a gap. Reading the emitted bytes needs no argument.

mod support;

use infra_compiler::InfraIr;

/// Every digest the fixture's secrets carry, and every key name they are stored under.
///
/// The digests are what the IR *does* hold, so they are the strongest available proxy for a leak:
/// a patch that carried a secret's content would almost certainly have carried the digest beside
/// it, and a patch that carried the digest alone would still be publishing a fact about a secret
/// that nothing outside the IR needs.
fn secret_material(ir: &InfraIr) -> (Vec<String>, Vec<String>) {
    let mut digests = Vec::new();
    let mut keys = Vec::new();
    for secret in ir.model.secrets.values() {
        for (key, digest) in &secret.keys {
            keys.push(key.clone());
            digests.push(digest.sha256.clone());
        }
    }
    (digests, keys)
}

#[test]
fn no_emitted_byte_carries_a_secrets_digest_or_key_name() {
    let ir = support::example_ir();
    let (digests, keys) = secret_material(&ir);
    // The fixture has to hold secrets for this to be a test of anything.
    assert!(
        !digests.is_empty(),
        "the committed observation carries no secret keys, so this scan proves nothing"
    );
    assert!(
        digests.iter().all(|digest| digest.len() == 64),
        "the IR is supposed to hold `{{sha256, length}}` and nothing else: {digests:?}"
    );

    let projection = infra_project::project(&support::example_spec(), &ir);
    let mut leaks = Vec::new();
    for (path, contents) in projection.artifacts() {
        for digest in &digests {
            if contents.contains(digest.as_str()) {
                leaks.push(format!("{path} carries the digest {digest}"));
            }
        }
        for key in &keys {
            // A key *name* is allowed to be named in an obligation — "create the secret
            // `agent-credentials`" is the whole point of that sentence — so the scan is about the
            // `data` shape a value would arrive in, not about the word appearing at all.
            if contents.contains(&format!("\"{key}\":")) && contents.contains("\"data\"") {
                leaks.push(format!("{path} carries a `data` block keyed by {key}"));
            }
        }
        assert!(
            !contents.contains("stringData"),
            "{path} writes a `stringData` block, which is where a plain secret value would go"
        );
    }
    assert!(leaks.is_empty(), "{}", leaks.join("\n"));
}

#[test]
fn a_dangling_secret_reference_is_owed_and_the_obligation_says_why_nothing_can_write_it() {
    // The one place the projection is *asked* for a secret: the fixture's `flaky-agent` reads a
    // secret nobody observed. The honest answer is an obligation that names the reason — the
    // snapshot holds a digest, not a value — rather than an empty secret manifest that would look
    // like progress and break the pod differently.
    let projection = infra_project::project(&support::example_spec(), &support::example_ir());
    let entry = projection
        .entries
        .iter()
        .find(|entry| entry.expectation == "shop-config-refs")
        .expect("the fixture has a dangling required reference");
    let infra_project::Disposition::Obligation(obligation) = &entry.disposition else {
        panic!("a dangling secret reference must never be generated: {entry:?}");
    };
    assert!(
        obligation.decision.contains("digest"),
        "the obligation says why nothing here can write the secret: {}",
        obligation.decision
    );
}
