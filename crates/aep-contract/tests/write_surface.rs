//! Invariant 14, enforced rather than stated: every mutation is a command.
//!
//! The contract's design gives state change exactly one door — `CommandService::execute` — because
//! validation, authorisation, protocol enforcement, idempotency, optimistic concurrency,
//! provenance, events, audit, correlation and causation all attach to that door, and a second door
//! is a second place for every one of them to be forgotten. Until this file that was "a property
//! of the contract's current shape": adding `fn delete(&self, ..)` to a trait here would compile,
//! pass every test and quietly hand backends a write path nothing audits.
//!
//! # What the test pins
//!
//! Backends are reached through this crate's traits and nothing else — the crate holds no state of
//! its own, so an inherent method or free function here cannot mutate a backend it never holds.
//! Enumerating every method of every public trait therefore enumerates the entire behavioural
//! surface an implementation can offer, and the expected map below says which of it may write:
//! `execute`, alone.
//!
//! **To whoever fails this test by adding a method:** if the new method mutates, it must not
//! exist — model the mutation as a command payload dispatched through `execute`, which is the
//! whole point of invariant 14. If it is a genuinely new *read*, add it to `QueryService`'s list
//! here in the same commit, with `AGENTS.md`'s invariant table untouched. If you are deliberately
//! opening a second write surface, that is a design change: change invariant 14 in `AGENTS.md`
//! first, and say why the ten concerns above are handled twice.

use std::path::Path;

/// Every public trait the contract may expose, with its exhaustive method list.
///
/// Sorted, because the extractor sorts what it finds.
const EXPECTED: &[(&str, &[&str])] = &[
    ("CommandService", &["execute"]),
    (
        "QueryService",
        &[
            "audit",
            "describe_type",
            "get",
            "history",
            "query",
            "relations",
            "resolve",
        ],
    ),
];

/// Every public trait declared in `text`, with its method names, sorted.
fn public_traits(text: &str) -> Vec<(String, Vec<String>)> {
    let mut found = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let Some(rest) = line.strip_prefix("pub trait ") else {
            continue;
        };
        let name = rest
            .split([' ', '<', ':', '{'])
            .next()
            .expect("a trait name")
            .to_owned();
        let mut depth = brace_depth(line);
        let mut methods = Vec::new();
        for body_line in lines.by_ref() {
            let trimmed = body_line.trim();
            if !trimmed.starts_with("//") && trimmed.starts_with("fn ") {
                let method = trimmed[3..]
                    .split(['(', '<'])
                    .next()
                    .expect("a method name")
                    .to_owned();
                methods.push(method);
            }
            depth += brace_depth(body_line);
            if depth <= 0 {
                break;
            }
        }
        methods.sort();
        found.push((name, methods));
    }
    found.sort();
    found
}

/// Opening minus closing braces on `line`, ignoring comment lines.
fn brace_depth(line: &str) -> i64 {
    if line.trim().starts_with("//") {
        return 0;
    }
    let mut depth = 0;
    for character in line.chars() {
        match character {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ => {}
        }
    }
    depth
}

#[test]
fn the_contract_has_one_write_path_and_it_is_the_command_boundary() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = Vec::new();
    let mut checked = 0;
    for entry in std::fs::read_dir(&directory).expect("the crate has sources") {
        let path = entry.expect("an entry").path();
        if path.extension().is_none_or(|it| it != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("a readable source file");
        found.extend(public_traits(&text));
        checked += 1;
    }
    found.sort();
    assert!(
        checked >= 6,
        "only {checked} source files were read; the scan is looking in the wrong place"
    );

    let expected: Vec<(String, Vec<String>)> = EXPECTED
        .iter()
        .map(|(name, methods)| {
            (
                (*name).to_owned(),
                methods.iter().map(|method| (*method).to_owned()).collect(),
            )
        })
        .collect();
    assert_eq!(
        found, expected,
        "invariant 14: the contract's behavioural surface has moved. Every mutation is a command \
         through `CommandService::execute` — the one write path — so a new trait or method is \
         either a command payload in disguise (model it as one), a new read (list it here, on \
         `QueryService`), or a second write surface (a design change: amend invariant 14 in \
         AGENTS.md first). Found {found:?}"
    );

    let (_, command_methods) = &found[0];
    assert_eq!(
        command_methods,
        &["execute".to_owned()],
        "the write path itself: `CommandService` carries exactly `execute`"
    );
}

#[test]
fn the_trait_extractor_reads_methods_and_not_prose() {
    let sample = "/// A service.\npub trait Sample {\n    /// Doc mentioning fn ghost( here.\n    type Command;\n\n    fn first(\n        &self,\n    ) -> u8;\n\n    fn second(&self) -> impl std::future::Future<Output = ()> {\n        async {}\n    }\n}\n\nstruct NotATrait {\n    field: u8,\n}\n";
    assert_eq!(
        public_traits(sample),
        vec![(
            "Sample".to_owned(),
            vec!["first".to_owned(), "second".to_owned()]
        )],
        "the extractor finds every method — required or default-bodied — and nothing else; a \
         default body is exactly how a second write path would slip past backends unnoticed"
    );

    let private = "trait Hidden {\n    fn inside(&self);\n}\n";
    assert!(
        public_traits(private).is_empty(),
        "a private trait is not part of the contract's surface"
    );
}
