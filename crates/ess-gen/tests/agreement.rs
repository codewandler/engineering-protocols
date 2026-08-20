//! Three projections, one answer per construct.
//!
//! `src/types.rs` is the type mapping this crate decided to have exactly once, and its module
//! documentation names this file as the check that keeps it that way. The defect it exists for is
//! recorded there: two projections carried private copies of the mapping, the copies disagreed, and
//! this repository published two contradictory answers to "what is a valid
//! `billing.invoice.InvoiceCreated`". Nothing in the build noticed, because every test asked whether
//! one projection's output was well formed and none asked whether two projections said the same
//! thing.
//!
//! So this file asks only that. For every construct more than one projection describes — a named
//! type, a command's input, an event's payload, an error's payload — it lifts the fragment each
//! projection publishes and requires the fragments to be equal.
//!
//! # The one difference that is not a disagreement
//!
//! A pointer's spelling. The same reference is `#/$defs/{name}` in `schema`, which writes
//! self-contained documents; `#/components/schemas/{name}` in `openapi`; and
//! `#/components/schemas/type.{name}` in `asyncapi`, whose table is keyed per kind so that an event
//! and a type sharing one name cannot replace each other. Each is right for the document it appears
//! in, so every `$ref` is reduced to the bare qualified name it resolves to and everything else is
//! compared as it stands.
//!
//! Nothing else is normalised, on purpose. A normalisation is a claim that a difference does not
//! matter, and each of the four differences this crate has already had to settle — `Optional`
//! outside a field, a union's layout, a map key's spelling, a `Duration`'s format — would have been
//! invisible under one more plausible-looking one.
//!
//! The document furniture around a `schema` message document is *removed* rather than normalised:
//! `$schema` names the dialect, `x-ess-provenance` names the build, and `$defs` is the definitions
//! table the other two projections keep once per document under `components.schemas`. None of the
//! three is part of what a document says a message looks like. The definitions are not thrown away —
//! they are exactly where this file reads `schema`'s fragment for each named type from, so that all
//! three fragments come from a definitions table rather than one of them from a document root.
//!
//! # What the billing example does not reach
//!
//! A construct only one projection publishes cannot be compared. `schema` emits a document for every
//! declared type; `openapi` and `asyncapi` emit what their own component's surface reaches. So
//! `billing.invoice.Payee`, `billing.invoice.Channel`, `billing.invoice.LineItem` and
//! `billing.invoice.CompanyRef` — declared, but reached only by an entity — are published once and
//! compared never. That leaves all four of the readings `src/types.rs` records as settled outside
//! what this file can see: a union's layout, an `Optional` outside a field, a map key's spelling and
//! a `Duration`'s format each need a construct no *message* in the example carries. What it does see
//! is the `Decimal` row — `billing.invoice.Money.amount` is reached by three constructs — and that
//! one row was enough to tell all three projections apart when the defect was measured. A command or
//! an event carrying a union, a `List<Optional<…>>`, a `Map<Integer, …>` or a `Duration` would bring
//! the other four in, and is the cheapest way to widen this test. The narrower assertions on those
//! four live in `tests/asyncapi.rs` and `tests/openapi.rs`, per projection, against a specification
//! written to reach them.
//!
//! # What has to agree, and what a difference would mean
//!
//! Every keyword, in both classes, and the classes are named because the difference between them is
//! the whole design:
//!
//! * An **assertion** — `type`, `format`, `pattern`, `required`, `additionalProperties`,
//!   `properties`, `items`, `propertyNames`, `enum`, `const`, `oneOf`, `anyOf`, `$ref`,
//!   `contentEncoding` — changes which bytes a document accepts. A projection that omits one
//!   publishes a contract *weaker* than the specification, and that is not a cosmetic difference:
//!   before this file was un-ignored, a service validating an event against the published `AsyncAPI`
//!   document accepted `{"amount": "abc", "currency": "EUR", "bogus": 1}` as a `billing.invoice.Money`
//!   while the JSON Schema for the same event refused it on all three counts. **This check is never
//!   relaxed.** If an assertion genuinely has to differ between two documents, that is a finding
//!   about the projections, to be argued and written down — not an exemption to add here.
//! * An **annotation** — `title`, `description`, `x-ess-name`, `x-ess-kind`, `x-ess-field`,
//!   `x-ess-map-key`, `x-ess-union-tag`, `x-ess-invariants` — changes only what a reader is shown.
//!
//! Annotations are compared just as strictly, and that is a decision rather than an oversight. The
//! case for exempting them is that an `AsyncAPI` reader and a JSON Schema reader are different
//! audiences who might want a different `title`. The case against, which won: every annotation here
//! is a **fact the model states** — what the construct is called, which kind it is, what its author
//! wrote about it, which invariants it satisfies — and none of them is a fact about the document it
//! appears in. A consumer is not one reader: it is a code generator, a documentation site and a
//! registry diff, and each of them is fed more than one of these files. When this projection answered
//! "which construct is this?" with `x-ess-type` and that one answered with `x-ess-name` plus
//! `x-ess-kind`, a generator reading both got two answers about one construct and no way to tell
//! which was authoritative. Two spellings of one fact is drift wearing the word "presentation".
//!
//! Where a projection wants a different presentation, the place for it is the furniture *around* the
//! schema — a channel, an operation, an `AsyncAPI` message, an `OpenAPI` request body — all of which
//! this file deliberately does not compare. The payload fragment is the model, and the model does not
//! change per reader.
//!
//! So a failure here is reported per class, because the two mean different things to whoever is
//! woken up: an assertion difference means the repository is publishing contracts that disagree about
//! which messages are valid, and an annotation difference means it is publishing one fact twice.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ess_compiler::ir::{ResolvedBody, ResolvedField, ResolvedTypeRef};
use ess_compiler::resolve::compile;
use ess_compiler::source::SourceMap;
use ess_compiler::EssIr;
use ess_domain::spec::{RawSpecFile, Specification};
use ess_domain::system::Source;
use ess_gen::artifact::{run, Artifact};
use ess_gen::asyncapi::AsyncApi;
use ess_gen::openapi::OpenApi;
use ess_gen::schema::JsonSchema;
use ess_gen::Generator;
use serde_json::Value;

/// The billing example's directory.
fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/billing")
        .canonicalize()
        .expect("the billing example exists")
}

/// Every `.yaml` file in the example, relative to it, in a stable order.
fn files() -> Vec<String> {
    let base = example();
    let mut found = Vec::new();
    let mut pending = vec![base.clone()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("the example is readable") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|it| it == "yaml") {
                found.push(
                    path.strip_prefix(&base)
                        .expect("inside the example")
                        .display()
                        .to_string(),
                );
            }
        }
    }
    assert!(!found.is_empty(), "the billing example holds no files");
    found.sort();
    found
}

/// The billing example, compiled.
///
/// From the files it lives in rather than a copy inlined here: the design document's own snippets
/// drifted three ways before anyone noticed, and a copy drifts the same way — which is review F7's
/// finding, and the reason this fixture is the same one every other test file in this directory uses.
fn billing() -> EssIr {
    let mut sources = SourceMap::new();
    let mut parsed = Vec::new();
    for label in files() {
        let text = std::fs::read_to_string(example().join(&label))
            .unwrap_or_else(|error| panic!("{label} is readable: {error}"));
        let raw = RawSpecFile::parse(&text)
            .unwrap_or_else(|error| panic!("{label} is well formed: {error}"));
        sources.insert(label.clone(), text);
        parsed.push((Source::new(label), raw));
    }
    let specification = Specification::assemble(parsed)
        .unwrap_or_else(|errors| panic!("the billing specification validates:\n{errors}"));
    compile(&specification, &sources)
        .unwrap_or_else(|diagnostics| panic!("the billing specification resolves:\n{diagnostics}"))
}

/// A named type the specification declares.
const NAMED_TYPE: &str = "named type";

/// What a command accepts.
const COMMAND_INPUT: &str = "command input";

/// What an event carries.
const EVENT_PAYLOAD: &str = "event payload";

/// What an error carries.
const ERROR_PAYLOAD: &str = "error payload";

/// Something a projection publishes a schema for.
///
/// Keyed by kind as well as by name, because the model's names are unique per kind and not across
/// kinds: an event and a named type may share a qualified name, and one table keyed by name alone
/// would compare an event's payload against a type's definition and report the difference as a
/// defect. It is the same reason `src/asyncapi.rs` prefixes its own schema keys.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Construct {
    /// Which of the four it is.
    kind: &'static str,
    /// The qualified name of the type or the message.
    name: String,
}

impl Construct {
    /// One construct.
    fn new(kind: &'static str, name: impl Into<String>) -> Self {
        Self {
            kind,
            name: name.into(),
        }
    }
}

impl std::fmt::Display for Construct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the {} `{}`", self.kind, self.name)
    }
}

/// Every fragment one projection publishes, keyed by construct.
struct Published {
    /// The projection's own name, as `--generator` spells it.
    projection: &'static str,
    /// One fragment per construct, with every pointer reduced to the name it resolves to.
    fragments: BTreeMap<Construct, Value>,
    /// Which artifact each fragment was read from, so a failure names a file a person can open.
    sources: BTreeMap<Construct, String>,
}

impl Published {
    /// Nothing published yet.
    fn new(projection: &'static str) -> Self {
        Self {
            projection,
            fragments: BTreeMap::new(),
            sources: BTreeMap::new(),
        }
    }

    /// Records the fragment this projection publishes for one construct.
    ///
    /// A projection publishes the same construct in more than one document — `billing.invoice.Money`
    /// is in five of `schema`'s files and in both `openapi` documents — and those copies being equal
    /// is the property this file checks between projections, one level down. So a second copy is
    /// compared with the first rather than silently overwriting it.
    fn record(&mut self, construct: Construct, fragment: Value, source: &str) {
        if let Some(first) = self.fragments.get(&construct) {
            let mut differences = Vec::new();
            let seen = self.sources[&construct].as_str();
            differences_between(&mut differences, "", (seen, first), (source, &fragment));
            assert!(
                differences.is_empty(),
                "the `{}` projection publishes two different schemas for {construct}, in `{seen}` \
                 and in `{source}`:\n{}",
                self.projection,
                all_lines(&differences)
            );
            return;
        }
        self.fragments.insert(construct.clone(), fragment);
        self.sources.insert(construct, source.to_owned());
    }
}

/// One value on one line, for a failure message a person reads top to bottom.
fn compact(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|error| panic!("a fragment serialises: {error}"))
}

/// The three pointer spellings, the longest first so `asyncapi`'s is not read as `openapi`'s.
const POINTERS: [&str; 3] = [
    "#/components/schemas/type.",
    "#/components/schemas/",
    "#/$defs/",
];

/// The qualified name a pointer resolves to.
///
/// A fourth spelling is a decision about the projections and gets made there, so it fails here
/// loudly rather than being folded into the normalisation — a normalisation that quietly accepts an
/// unknown pointer is a normalisation that can hide a reference to the wrong table.
fn resolved(pointer: &Value) -> String {
    let text = pointer
        .as_str()
        .unwrap_or_else(|| panic!("a `$ref` is a string, not {}", compact(pointer)));
    for prefix in POINTERS {
        if let Some(name) = text.strip_prefix(prefix) {
            return name.to_owned();
        }
    }
    panic!(
        "`{text}` is not one of the pointer spellings this test knows ({}); a fourth is a decision \
         about the projections, not something to normalise away",
        POINTERS.join(", ")
    )
}

/// The same fragment with every pointer reduced to the qualified name it resolves to.
fn normalised(fragment: &Value) -> Value {
    match fragment {
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(keyword, value)| {
                    let value = if keyword == "$ref" {
                        Value::String(resolved(value))
                    } else {
                        normalised(value)
                    };
                    (keyword.clone(), value)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(normalised).collect()),
        other => other.clone(),
    }
}

/// The keywords that change which bytes a document accepts.
///
/// Listed rather than derived, because "does this keyword assert anything" is a fact about the
/// dialect and not about this repository: `contentEncoding` is an annotation in 2020-12 and is in
/// here anyway, since the mapping pairs it with a `pattern` and a reader looking for the encoding
/// rule finds both under one heading. A keyword in neither list is treated as an assertion, which is
/// the safe default and which `every_keyword_the_projections_publish_is_classified` makes loud rather
/// than silent.
const ASSERTIONS: [&str; 14] = [
    "$ref",
    "additionalProperties",
    "anyOf",
    "const",
    "contentEncoding",
    "enum",
    "format",
    "items",
    "oneOf",
    "pattern",
    "properties",
    "propertyNames",
    "required",
    "type",
];

/// The keywords that change only what a reader is shown.
///
/// Compared exactly as strictly as [`ASSERTIONS`]. The argument is in this file's module
/// documentation: each of these is a fact the *model* states, not a fact about the document carrying
/// it, and one fact with two spellings is drift.
const ANNOTATIONS: [&str; 8] = [
    "description",
    "title",
    "x-ess-field",
    "x-ess-invariants",
    "x-ess-kind",
    "x-ess-map-key",
    "x-ess-name",
    "x-ess-union-tag",
];

/// The keyword whose children are property names rather than keywords.
const PROPERTIES: &str = "properties";

/// What a difference at one keyword costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Class {
    /// The two documents disagree about which messages are valid.
    Assertion,
    /// The two documents state one fact two ways.
    Annotation,
}

impl Class {
    /// The word a failure names this class with.
    fn label(self) -> &'static str {
        match self {
            Self::Assertion => "assertion",
            Self::Annotation => "annotation",
        }
    }

    /// What a reader woken up by a difference in this class is looking at.
    fn means(self) -> &'static str {
        match self {
            Self::Assertion => {
                "these keywords change what a document accepts, so a projection missing one \
                 publishes a contract weaker than the specification: a message this repository \
                 refuses in one published file is accepted in another. This check is not relaxed to \
                 make a failure go away — an assertion that genuinely must differ is a finding about \
                 the projections"
            }
            Self::Annotation => {
                "these keywords change only what a reader is shown, and they are compared anyway: \
                 each one is a fact the model states — the construct's name, its kind, its author's \
                 words, its invariants — rather than a fact about the document carrying it, so two \
                 spellings of it means a consumer reading both files gets two answers about one \
                 construct. Per-document presentation belongs in the furniture around the schema, \
                 which this file does not compare"
            }
        }
    }
}

/// Which class a difference at this path belongs to.
///
/// The deepest keyword decides, so `properties.amount.description` is an annotation and
/// `properties.amount.format` is an assertion. Read forwards rather than backwards precisely because
/// of `properties`, whose children are field names: a field whose wire name is `title` would
/// otherwise make its own absence look like a difference of presentation. A path with no keyword
/// either list names — a keyword nobody has classified yet — is an assertion, because "we do not know
/// whether this changes what the document accepts" has one safe answer.
fn class_of(at: &str) -> Class {
    let mut class = Class::Assertion;
    let mut naming = false;
    for segment in at.split('.') {
        // `oneOf[1]` is the keyword `oneOf`; an index carries the class of the keyword above it.
        let keyword = segment.split('[').next().unwrap_or(segment);
        if naming {
            naming = false;
        } else if keyword == PROPERTIES {
            naming = true;
            class = Class::Assertion;
        } else if ANNOTATIONS.contains(&keyword) {
            class = Class::Annotation;
        } else if ASSERTIONS.contains(&keyword) {
            class = Class::Assertion;
        }
    }
    class
}

/// One keyword two fragments disagree about.
#[derive(Debug)]
struct Difference {
    /// What the difference costs, which decides how the failure reads.
    class: Class,
    /// The keyword and both values, as a line a person reads top to bottom.
    line: String,
}

/// Every keyword where two fragments disagree, classified and rendered.
///
/// Every difference rather than the first: a projection that differs in `description` usually
/// differs in three more places, and a reader woken up by this failure wants the whole disagreement
/// rather than one keyword and another run.
fn differences_between(
    into: &mut Vec<Difference>,
    at: &str,
    left: (&str, &Value),
    right: (&str, &Value),
) {
    let (from_left, on_left) = left;
    let (from_right, on_right) = right;
    let below = |name: &str| {
        if at.is_empty() {
            name.to_owned()
        } else {
            format!("{at}.{name}")
        }
    };
    let here = if at.is_empty() {
        "the whole fragment".to_owned()
    } else {
        format!("`{at}`")
    };
    let found = |into: &mut Vec<Difference>, path: &str, line: String| {
        into.push(Difference {
            class: class_of(path),
            line,
        });
    };

    match (on_left, on_right) {
        (Value::Object(first), Value::Object(second)) => {
            let keywords: BTreeSet<&String> = first.keys().chain(second.keys()).collect();
            for keyword in keywords {
                match (first.get(keyword), second.get(keyword)) {
                    (Some(a), Some(b)) => {
                        differences_between(into, &below(keyword), (from_left, a), (from_right, b));
                    }
                    (Some(a), None) => found(
                        into,
                        &below(keyword),
                        format!(
                            "    `{}`: only `{from_left}` publishes it, as {}",
                            below(keyword),
                            compact(a)
                        ),
                    ),
                    (None, Some(b)) => found(
                        into,
                        &below(keyword),
                        format!(
                            "    `{}`: only `{from_right}` publishes it, as {}",
                            below(keyword),
                            compact(b)
                        ),
                    ),
                    (None, None) => unreachable!("a keyword came from one of the two objects"),
                }
            }
        }
        (Value::Array(first), Value::Array(second)) if first.len() == second.len() => {
            for (index, (a, b)) in first.iter().zip(second).enumerate() {
                differences_between(
                    into,
                    &format!("{at}[{index}]"),
                    (from_left, a),
                    (from_right, b),
                );
            }
        }
        (a, b) if a != b => found(
            into,
            at,
            format!(
                "    {here}: `{from_left}` publishes {}, `{from_right}` publishes {}",
                compact(a),
                compact(b)
            ),
        ),
        _ => {}
    }
}

/// The differences of one class, as lines, or nothing when there are none.
fn of_class(differences: &[Difference], class: Class) -> Vec<&str> {
    differences
        .iter()
        .filter(|difference| difference.class == class)
        .map(|difference| difference.line.as_str())
        .collect()
}

/// Every difference as lines, whatever its class.
fn all_lines(differences: &[Difference]) -> String {
    differences
        .iter()
        .map(|difference| difference.line.as_str())
        .collect::<Vec<&str>>()
        .join("\n")
}

/// Every artifact one projection produces, keyed by path.
fn artifacts(generator: &dyn Generator, ir: &EssIr) -> BTreeMap<String, Artifact> {
    run(generator, ir).expect("no two artifacts of one projection claim one path")
}

/// The keywords a `schema` document carries about the file rather than about the payload.
const FURNITURE: [&str; 3] = ["$schema", "$defs", "x-ess-provenance"];

/// What the `schema` projection publishes, per construct.
fn published_by_schema(ir: &EssIr) -> Published {
    let mut out = Published::new("schema");
    for (path, artifact) in artifacts(&JsonSchema, ir) {
        let document: Value = serde_json::from_str(&artifact.contents)
            .unwrap_or_else(|error| panic!("{path} is JSON: {error}"));

        // Read from `$defs` rather than from the type document's root, because that is the same
        // place the other two projections' fragments come from: a definitions table.
        if let Some(defs) = document.get("$defs").and_then(Value::as_object) {
            for (name, definition) in defs {
                out.record(
                    Construct::new(NAMED_TYPE, name),
                    normalised(definition),
                    &path,
                );
            }
        }

        // A message document's root *is* the payload. A type document's root is a pointer into its
        // own `$defs` and carries no `x-ess-kind`, so it contributes nothing here.
        if let Some(construct) = message_construct(&document) {
            let mut payload = document.clone();
            let fields = payload.as_object_mut().expect("a document is an object");
            for keyword in FURNITURE {
                fields.remove(keyword);
            }
            out.record(construct, normalised(&payload), &path);
        }
    }
    out
}

/// Which message a `schema` document describes, when it describes one.
fn message_construct(document: &Value) -> Option<Construct> {
    let name = document.get("x-ess-name")?.as_str()?;
    let kind = match document.get("x-ess-kind")?.as_str()? {
        "command-input" => COMMAND_INPUT,
        "event-payload" => EVENT_PAYLOAD,
        "error-payload" => ERROR_PAYLOAD,
        other => panic!(
            "`{other}` is a message kind this test does not know; a new one needs a decision about \
             which projections publish it before it can be compared"
        ),
    };
    Some(Construct::new(kind, name))
}

/// What the `openapi` projection publishes, per construct.
///
/// The `components.schemas` keys are formed by `src/openapi.rs` out of the IR's own names, so they
/// are looked up from the IR rather than parsed back out of a key: a named type whose last segment
/// is `Input` would otherwise be read as some command's request body. A key this table does not
/// claim is an outcome response, which no other projection publishes and this file therefore has
/// nothing to compare it with.
fn published_by_openapi(ir: &EssIr) -> Published {
    let mut wanted = BTreeMap::new();
    for name in ir.types.keys() {
        wanted.insert(
            name.to_string(),
            Construct::new(NAMED_TYPE, name.to_string()),
        );
    }
    for name in ir.commands.keys() {
        wanted.insert(
            format!("{name}.Input"),
            Construct::new(COMMAND_INPUT, name.to_string()),
        );
    }
    for name in ir.errors.keys() {
        wanted.insert(
            format!("{name}.Error"),
            Construct::new(ERROR_PAYLOAD, name.to_string()),
        );
    }
    under_components(&OpenApi, "openapi", ir, &wanted)
}

/// What the `asyncapi` projection publishes, per construct.
fn published_by_asyncapi(ir: &EssIr) -> Published {
    let mut wanted = BTreeMap::new();
    for name in ir.types.keys() {
        wanted.insert(
            format!("type.{name}"),
            Construct::new(NAMED_TYPE, name.to_string()),
        );
    }
    for name in ir.events.keys() {
        wanted.insert(
            format!("event.{name}"),
            Construct::new(EVENT_PAYLOAD, name.to_string()),
        );
    }
    under_components(&AsyncApi, "asyncapi", ir, &wanted)
}

/// Every fragment a YAML projection publishes under `components.schemas`, keyed by construct.
fn under_components(
    generator: &dyn Generator,
    projection: &'static str,
    ir: &EssIr,
    wanted: &BTreeMap<String, Construct>,
) -> Published {
    let mut out = Published::new(projection);
    for (path, artifact) in artifacts(generator, ir) {
        let document: Value = serde_yaml::from_str(&artifact.contents)
            .unwrap_or_else(|error| panic!("{path} is YAML: {error}"));
        let schemas = document
            .get("components")
            .and_then(|components| components.get("schemas"))
            .and_then(Value::as_object);
        for (key, fragment) in schemas.into_iter().flatten() {
            if let Some(construct) = wanted.get(key) {
                out.record(construct.clone(), normalised(fragment), &path);
            }
        }
    }
    out
}

/// The three contract projections, each harvested.
fn projections(ir: &EssIr) -> [Published; 3] {
    [
        published_by_schema(ir),
        published_by_openapi(ir),
        published_by_asyncapi(ir),
    ]
}

/// Every construct more than one projection describes, with the projections that describe it.
fn shared(published: &[Published]) -> BTreeMap<Construct, Vec<&Published>> {
    let mut out: BTreeMap<Construct, Vec<&Published>> = BTreeMap::new();
    for projection in published {
        for construct in projection.fragments.keys() {
            out.entry(construct.clone()).or_default().push(projection);
        }
    }
    out.retain(|_, holders| holders.len() > 1);
    out
}

/// How many constructs the billing example puts in more than one projection.
///
/// A floor rather than an exact count, so that widening a projection's reach does not fail here — but
/// a floor, and not nothing, because a harvest that quietly stopped finding fragments would turn the
/// agreement check into a test of zero constructs that passes.
const SHARED_FLOOR: usize = 11;

#[test]
fn every_projection_publishes_the_same_schema_for_a_construct_more_than_one_of_them_describes() {
    let ir = billing();
    let published = projections(&ir);
    let shared = shared(&published);

    assert!(
        shared.len() >= SHARED_FLOOR,
        "only {} constructs are published by more than one projection, so this test compares \
         almost nothing; the harvest has stopped finding fragments",
        shared.len()
    );

    let mut pairs = 0usize;
    let mut findings: Vec<String> = Vec::new();
    let mut counted: BTreeMap<Class, usize> = BTreeMap::new();
    for (construct, holders) in &shared {
        for (index, left) in holders.iter().enumerate() {
            for right in &holders[index + 1..] {
                pairs += 1;
                let mut differences = Vec::new();
                differences_between(
                    &mut differences,
                    "",
                    (left.projection, &left.fragments[construct]),
                    (right.projection, &right.fragments[construct]),
                );
                if differences.is_empty() {
                    continue;
                }
                let mut report = vec![format!(
                    "  {construct} differs between `{}` (`{}`) and `{}` (`{}`):",
                    left.projection,
                    left.sources[construct],
                    right.projection,
                    right.sources[construct]
                )];
                // Grouped by class, because the two are different failures: one says the published
                // contracts disagree about which messages are valid, the other says they state one
                // fact two ways. What each class costs is spelt out once, below the findings.
                for class in [Class::Assertion, Class::Annotation] {
                    let lines = of_class(&differences, class);
                    if lines.is_empty() {
                        continue;
                    }
                    *counted.entry(class).or_default() += lines.len();
                    report.push(format!(
                        "    {} {} keyword(s):\n{}",
                        lines.len(),
                        class.label(),
                        lines.join("\n")
                    ));
                }
                findings.push(report.join("\n"));
            }
        }
    }

    // The counts a reader of a green run wants: `cargo test -- --nocapture` prints them, so "this
    // passed" and "this compared something" are the same observation.
    println!(
        "compared {} shared constructs over {pairs} projection pairs, keyword by keyword",
        shared.len()
    );

    let costs: Vec<String> = counted
        .keys()
        .map(|class| format!("An {} difference: {}.", class.label(), class.means()))
        .collect();
    assert!(
        findings.is_empty(),
        "{} of {pairs} projection pairs publish a different schema for the same construct \
         ({} differing assertion keywords, {} differing annotation keywords), so this repository \
         publishes {} contradictory contracts:\n\n{}\n\n{}",
        findings.len(),
        counted.get(&Class::Assertion).copied().unwrap_or_default(),
        counted.get(&Class::Annotation).copied().unwrap_or_default(),
        findings.len(),
        findings.join("\n\n"),
        costs.join("\n\n")
    );
}

/// Every keyword any projection publishes in a keyword position, with the property names left out.
///
/// The direct children of `properties` are field names an author chose, not keywords, so they are
/// skipped and their schemas are walked instead. Everything else in these fragments is keyed by
/// keyword.
fn keywords_published(published: &[Published]) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for projection in published {
        for fragment in projection.fragments.values() {
            collect_keywords(fragment, &mut found);
        }
    }
    found
}

/// Adds every keyword this fragment uses, and every keyword the schemas inside it use.
fn collect_keywords(fragment: &Value, into: &mut BTreeSet<String>) {
    match fragment {
        Value::Object(fields) => {
            for (keyword, value) in fields {
                into.insert(keyword.clone());
                if keyword == PROPERTIES {
                    for schema in value.as_object().into_iter().flatten().map(|(_, it)| it) {
                        collect_keywords(schema, into);
                    }
                } else {
                    collect_keywords(value, into);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_keywords(item, into);
            }
        }
        _ => {}
    }
}

#[test]
fn every_keyword_the_projections_publish_is_classified_as_an_assertion_or_an_annotation() {
    // Without this, a keyword nobody classified would be compared under the fail-safe default and
    // the taxonomy above would quietly stop describing the output — which is the same failure as the
    // mapping quietly stopping to describe the model. A new keyword is a decision: whether it
    // changes what a document accepts is exactly what its author has to answer, here, in writing.
    let ir = billing();
    let published = projections(&ir);
    let found = keywords_published(&published);

    assert!(
        found.len() >= ASSERTIONS.len(),
        "only {} keywords were found across every published fragment, which is fewer than the {} \
         this file classifies as assertions; the harvest is not reading the projections' output: \
         {found:?}",
        found.len(),
        ASSERTIONS.len()
    );

    let unclassified: Vec<&String> = found
        .iter()
        .filter(|keyword| {
            !ASSERTIONS.contains(&keyword.as_str()) && !ANNOTATIONS.contains(&keyword.as_str())
        })
        .collect();
    assert!(
        unclassified.is_empty(),
        "{unclassified:?} are published and classified as neither an assertion nor an annotation. \
         They are compared as assertions by default, which is safe and silent; decide which class \
         each belongs in and say so in `ASSERTIONS` or `ANNOTATIONS`"
    );

    // Both classes, so that the comparison is not secretly about one of them. An assertion-only
    // harvest would let every annotation drift unwatched, and the reverse would be worse.
    for class in [Class::Assertion, Class::Annotation] {
        let named: Vec<&String> = found
            .iter()
            .filter(|keyword| class_of(keyword) == class)
            .collect();
        assert!(
            !named.is_empty(),
            "no published keyword is a {class:?}, so the agreement check is not comparing that \
             class at all: {found:?}"
        );
    }
}

#[test]
fn the_agreement_check_compares_the_constructs_the_defect_was_about_rather_than_nothing() {
    let ir = billing();
    let published = projections(&ir);
    let shared = shared(&published);

    assert!(
        shared.len() >= SHARED_FLOOR,
        "only {} constructs are published by more than one projection, so the agreement check \
         compares almost nothing:\n{:?}",
        shared.len(),
        shared.keys().collect::<Vec<_>>()
    );

    // The three the recorded defect was about, each named rather than counted: `InvoiceCreated` is
    // the event two projections gave contradictory answers for, `Money` is the one construct all
    // three describe, and `CreateInvoice`'s input is what a caller validates a request body against.
    for (construct, expected) in [
        (
            Construct::new(EVENT_PAYLOAD, "billing.invoice.InvoiceCreated"),
            vec!["asyncapi", "schema"],
        ),
        (
            Construct::new(NAMED_TYPE, "billing.invoice.Money"),
            vec!["asyncapi", "openapi", "schema"],
        ),
        (
            Construct::new(COMMAND_INPUT, "billing.invoice.CreateInvoice"),
            vec!["openapi", "schema"],
        ),
    ] {
        let holders = shared
            .get(&construct)
            .unwrap_or_else(|| panic!("{construct} is published by more than one projection"));
        let mut names: Vec<&str> = holders.iter().map(|it| it.projection).collect();
        names.sort_unstable();
        assert_eq!(
            names, expected,
            "{construct} is compared across the wrong set of projections"
        );
    }
}

/// Every newtype the specification declares, by qualified name.
fn newtypes(ir: &EssIr) -> BTreeMap<String, String> {
    ir.types
        .iter()
        .filter_map(|(name, declared)| match &declared.body {
            ResolvedBody::Newtype { of, .. } => Some((name.to_string(), of.to_string())),
            _ => None,
        })
        .collect()
}

/// Every field whose type *is* a newtype, with the construct carrying it and its wire name.
///
/// Only a direct reference, not a `List` or a `Map` of one: at those positions the published schema
/// is an array or an object and the reference sits inside it, which is a different assertion from
/// the one this drives.
fn newtype_fields(ir: &EssIr) -> Vec<(Construct, String, String)> {
    let declared = newtypes(ir);
    let mut out = Vec::new();
    let mut carried = |construct: &Construct, fields: &[ResolvedField]| {
        for field in fields {
            let ResolvedTypeRef::Declared { name } = field.type_ref.required() else {
                continue;
            };
            let target = ir.named_type(name).name.to_string();
            if declared.contains_key(&target) {
                let wire = field
                    .naming
                    .wire
                    .clone()
                    .unwrap_or_else(|| field.name.clone());
                out.push((construct.clone(), wire, target));
            }
        }
    };

    for (name, command) in &ir.commands {
        carried(
            &Construct::new(COMMAND_INPUT, name.to_string()),
            &command.input,
        );
    }
    for (name, event) in &ir.events {
        carried(
            &Construct::new(EVENT_PAYLOAD, name.to_string()),
            &event.fields,
        );
    }
    for (name, error) in &ir.errors {
        carried(
            &Construct::new(ERROR_PAYLOAD, name.to_string()),
            &error.fields,
        );
    }
    for (name, kind) in &ir.types {
        if let ResolvedBody::Struct { fields, .. } = &kind.body {
            carried(&Construct::new(NAMED_TYPE, name.to_string()), fields);
        }
    }
    out
}

/// How many positions one collapse check looked at, and everywhere it found the collapse.
type Checked = (usize, Vec<String>);

/// Every newtype definition that says only what JSON type its value is.
///
/// That *is* the collapse `docs/plan/ess-wave-3-projections.md` names: `billing.invoice.Email` and
/// `billing.email.EmailAddress` are both a `String` underneath, so a document publishing either as
/// `{"type": "string"}` no longer says which of the two it holds, and a consumer of that schema can
/// put an invoice's email where a delivery address belongs.
fn anonymous_definitions(published: &[Published], declared: &BTreeMap<String, String>) -> Checked {
    let mut checked = 0;
    let mut found = Vec::new();
    for projection in published {
        for name in declared.keys() {
            let construct = Construct::new(NAMED_TYPE, name.clone());
            let Some(fragment) = projection.fragments.get(&construct) else {
                continue;
            };
            checked += 1;
            if fragment
                .as_object()
                .is_some_and(|it| it.len() == 1 && it.contains_key("type"))
            {
                found.push(format!(
                    "  `{}` publishes {construct} as {}, which names no type at all",
                    projection.projection,
                    compact(fragment)
                ));
            }
        }
    }
    (checked, found)
}

/// Every pair of newtypes over one representation that some projection publishes as one schema.
///
/// Two definitions that are byte-identical are one definition to anything generated from the
/// document: a code generator emits a single class for both, and the distinction is gone downstream
/// even though every document validated.
fn merged_definitions(published: &[Published], declared: &BTreeMap<String, String>) -> Checked {
    let mut checked = 0;
    let mut found = Vec::new();
    for (left, right) in same_representation(declared) {
        for projection in published {
            let first = projection
                .fragments
                .get(&Construct::new(NAMED_TYPE, left.clone()));
            let second = projection
                .fragments
                .get(&Construct::new(NAMED_TYPE, right.clone()));
            let (Some(first), Some(second)) = (first, second) else {
                continue;
            };
            checked += 1;
            if first == second {
                found.push(format!(
                    "  `{}` publishes `{left}` and `{right}` as the same schema, {}, so the one \
                     distinction the specification declares them for is not in the document",
                    projection.projection,
                    compact(first)
                ));
            }
        }
    }
    (checked, found)
}

/// Every field of a newtype a projection publishes as something other than a reference to it.
///
/// Inlining the representation is the same collapse one position out: the property would accept a
/// bare string and say nothing about which newtype the author declared. A reference to a definition
/// the projection does not publish is the same failure from the other end — the document names the
/// type and then does not describe it.
fn inlined_uses(published: &[Published], ir: &EssIr) -> Checked {
    let mut checked = 0;
    let mut found = Vec::new();
    for (construct, wire, target) in newtype_fields(ir) {
        for projection in published {
            let Some(fragment) = projection.fragments.get(&construct) else {
                continue;
            };
            checked += 1;
            let property = fragment
                .get("properties")
                .and_then(|properties| properties.get(&wire));
            let reference = property
                .and_then(|schema| schema.get("$ref"))
                .and_then(Value::as_str);
            if reference != Some(target.as_str()) {
                found.push(format!(
                    "  `{}` publishes {construct}'s `{wire}` as {}, not as a reference to \
                     `{target}`",
                    projection.projection,
                    property.map_or_else(|| "nothing".to_owned(), compact)
                ));
            } else if !projection
                .fragments
                .contains_key(&Construct::new(NAMED_TYPE, target.clone()))
            {
                found.push(format!(
                    "  `{}` refers {construct}'s `{wire}` to `{target}` and publishes no \
                     definition for it, so the reference resolves to nothing",
                    projection.projection
                ));
            }
        }
    }
    (checked, found)
}

#[test]
fn no_projection_collapses_a_newtype_into_the_representation_it_wraps() {
    let ir = billing();
    let published = projections(&ir);
    let declared = newtypes(&ir);
    assert!(
        !declared.is_empty(),
        "the billing example declares no newtype, so this test asserts nothing"
    );

    let mut findings = Vec::new();
    // Counted per check rather than in one total, so that a check which stopped looking at anything
    // fails here instead of being carried by the other two.
    for (what, (checked, found)) in [
        (
            "newtype definitions",
            anonymous_definitions(&published, &declared),
        ),
        (
            "pairs of newtypes over one representation",
            merged_definitions(&published, &declared),
        ),
        ("fields of a newtype", inlined_uses(&published, &ir)),
    ] {
        assert!(
            checked >= declared.len(),
            "only {checked} {what} were checked, against {} newtypes the specification declares, \
             so this check is not looking at the projections' output",
            declared.len()
        );
        findings.extend(found);
    }

    assert!(
        findings.is_empty(),
        "{} newtype positions collapse into the representation the model declared them apart \
         from:\n{}",
        findings.len(),
        findings.join("\n")
    );
}

/// Every pair of newtypes wrapping the same representation, in a stable order.
fn same_representation(declared: &BTreeMap<String, String>) -> Vec<(String, String)> {
    let mut by_representation: BTreeMap<&String, Vec<&String>> = BTreeMap::new();
    for (name, representation) in declared {
        by_representation
            .entry(representation)
            .or_default()
            .push(name);
    }
    let mut out = Vec::new();
    for names in by_representation.values() {
        for (index, left) in names.iter().enumerate() {
            for right in &names[index + 1..] {
                out.push(((*left).clone(), (*right).clone()));
            }
        }
    }
    out
}
