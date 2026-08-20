# Changelog

Notable changes to `engineering-protocols`. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html), where a **major**
version is a breaking change to a protocol's semantics, not merely to a Rust API.

Entries record what changed for someone using the protocol. Rationale that does not fit in a line
belongs in the commit message or in `docs/design/`.

## [Unreleased]

## [0.4.0-ess-wave-4] — 2026-08-20

### Changed

- **A command outcome that changes an entity must say which instance.** `creates:`, `moves:` and
  `updates:` named the entity; `instance:` now names the field carrying its identity, and an outcome
  with a subject and no instance is refused. **This will refuse a specification that used to be
  accepted** — every state-changing outcome needs one word added.

  The reason is a measurement rather than a preference. A generated conformance suite could not test
  a single lifecycle transition without it: `PayInvoice` settles *an* invoice, and nothing connected
  its input to that invoice's identity, so twenty-eight scenarios across the two example
  specifications refused to generate rather than fabricate an id — and a fabricated id fails a
  *correct* implementation, which is worse than generating nothing. With the link declared, those
  twenty-eight became scenarios.

  It is declared rather than inferred, because inference has no answer when a command carries two
  fields of the identity's type and no answer when it carries none — and because an inferred link
  would silently change which scenarios exist when someone adds an unrelated field, while stored
  conformance results are keyed on exactly those names. It hangs on the outcome, not the command,
  for the reason the subject does: a command's branches disagree about what they touch, and a
  command-level key would attach an instance to a refusal.

  `creates:` is the exception and points at an event rather than the input: a created instance does
  not exist when the caller calls, so its identity is published rather than supplied.

### Added

- **A specification generates its own conformance scenarios.** All five families: one per reachable
  command outcome with the refusal branch asserting the success event did *not* occur, an externally
  decided branch reached by configuring the fault rather than by an input, a lifecycle transition
  proved and an illegal one refused, an entity's invariants checked after each state-changing command,
  and a binding checked for its mapping, its delivery guarantee and its failure policy. The normative
  example yields twenty-seven scenarios and the oracle fixture thirty-one. Nothing executes them yet.
- **The generated suite is checked against implementations that are deliberately wrong.** Ten faults,
  each injected one at a time: a wrong event, an accepted invalid amount, an illegal transition
  allowed, a dropped binding, a swapped mapping, a stale read-your-writes view, an ignored external
  outcome. Seven are caught by the scenario that exists to catch them — named, not merely "the run
  went red" — and the matrix asserts each fault's blast radius against an allowance, so a suite that
  starts over-reaching fails rather than looking thorough.
- **A command can say what happens when it is attempted in the wrong state**, and an author writes
  only the error. `wrong_state: true` with an `error:` is a fourth kind of outcome beside a guarded
  branch, a default branch and an externally decided one. The *states* are not written down: the
  lifecycle already says which states each transition may be taken from, so everything else is wrong
  by construction — add a `from:` to a transition and the branch narrows without anyone editing a
  second list.

  Until now a generated suite could only check that something went wrong, not that the right thing
  went wrong. An implementation that refuses with the wrong error passed all twenty-seven scenarios of
  the normative example; it now fails the scenario that exists to catch it. Omitting `wrong_state:` is
  still valid — the scenario is still generated, and the suite says plainly that the specification
  declares no answer for it.

  For anyone generating contracts: the branch surfaces in OpenAPI as `409`, not `422` — the caller's
  request was well formed, and telling them to fix it would send them looking for a mistake they did
  not make.
- **Two of those three faults are now caught.** A command may no longer announce an event belonging to
  a branch it did not take: every event the specification declares and the branch does not emit is
  asserted absent, scoped to that invocation. And a read-your-writes view whose command returned no
  consistency token is no longer quietly read at whatever is current — the check fails, naming the
  command that owes the token, because a weaker read that passes is a skip wearing a pass's clothes.
- **An event's payload is checked for shape.** Every declared field must be present and of its
  declared type, down to the leaves. Its *value* still is not, and cannot be: nothing in the model
  relates a command's input to an emitted event's payload, so `InvoiceCreated.amount` matching
  `CreateInvoice.amount` is a coincidence of field names rather than something the specification says.
  Closing that needs a construct in the shape `mapping:` already has, and until then the fault stays
  recorded as uncaught with its reason narrowed.
- **A view assertion names the instance the scenario acted on**, rather than meaning "the view holds
  some row". The weaker form was correct only because scenarios are isolated, and would have passed
  against a shared target for reasons unrelated to the rule being tested.
- **Three faults are caught by nothing, and the matrix records that too.** An event may be published
  with any payload, and a command may announce an event belonging to a branch it did not take, because
  synthesis asserts an event by name and writes no payload; and a target that returns no consistency
  token gets a weaker read instead of a reported failure. Each is recorded as an uncaught fault with
  the reason, and the test asserts it is *still* uncaught — so closing one of these holes breaks the
  row rather than being quietly forgotten.
- **`protocol ess conform`** — `synthesize` writes a suite from a specification, `run` executes one
  against an implementation. It can run the two reference implementations this repository ships, and
  its help says outright that it cannot run yours, with the four-line adapter recipe rather than an
  implication that more is there. Exit codes distinguish the three answers that matter: `0` conformant,
  `1` the implementation contradicted the specification or could not expose something required, `3` the
  run could not be carried out at all — because telling a harness the system is wrong when nobody found
  out is its own kind of lie.
- **The generated suites are committed and drift-checked**, under `suites/generated/`, as a seventh
  step of the gate and a CI job of its own. They sit beside the projections rather than inside them,
  because that tree has one owner and an orphan scan that deletes what its owner did not produce — two
  writers there would each delete the other's committed contract.

  The committed index also lists every construct that got **no** scenario, with the reason. A suite
  quietly holding fewer checks than it used to is the one failure a passing run cannot show you, and
  now it is a line in a diff.
- **A generated suite runs against an implementation.** A `ConformanceTarget` offers nine methods,
  each traceable to something the specification declares — execute a command, query a view, observe
  events, configure an externally decided outcome, redeliver an event, isolate a scenario. There is no
  assertion method and no escape hatch: if a step cannot be executed through concepts the model
  declares, that is a finding about the model rather than a method on the trait. All twenty-seven
  scenarios of the normative example pass against a hand-written reference implementation, and two
  runs produce byte-identical reports, because the runner owns the clock and the id source and nothing
  beneath it reaches for an ambient one.
- **A scenario the target cannot observe fails the run rather than passing quietly.** `unsupported` is
  its own status beside `passed`, `failed` and `error`, and a required scenario that ends in it makes
  conformance fail — a skip that reads as a pass is how a suite comes to certify what it never checked.
- **A binding's promises are each a test.** The mapping is asserted field by field, so a swap between
  two same-typed fields is caught rather than passing. `at_least_once` delivers the event twice and
  requires the consequence to survive it — not to happen exactly once, which is the assertion that
  looks right and fails a correct at-least-once handler. An escalation asserts the event the model
  now requires it to name.
- **`on_failure: drop` generates a refusal rather than a scenario**, saying so in the suite: a policy
  that gives up silently publishes nothing, so there is nothing to assert, and the hint says to write
  `escalate:` if it has to be provable. The refusal is the honest output — a scenario would have to
  invent an observation the specification declines to make.

- **`protocol ess graph --format mermaid`.** The system graph as a Mermaid flowchart, unfenced, so it
  can be piped into a Markdown file, a docs site or a pull request without going through the generated
  documentation tree. `dot`, `json` and `yaml` are the other spellings; `--format text` still means DOT
  and is kept as an alias of it.

### Fixed

- **An artifact evidence record could not be written in a document at all.** The evidence envelope is
  tagged by `kind`, and this one kind of record also had a field called `kind` — so the parser
  consumed the key as the tag and then reported the field it had just consumed as missing. Every
  attempt failed with `missing field 'kind'`, however it was written. The field is `artifact_kind` on
  the wire now.

  The consequence was wider than one record type: `design-by-contract` and `preserve-evidence` both
  require artifact evidence, so no `development.critical` task could satisfy either through a
  document, and none could reach `implement`. The variant existed, was documented, appeared in the
  published schema, and was unreachable from the one place a person writes evidence.
- **The CLI and the documentation page were drawing two different system graphs.** The command line
  showed no actors and no grants at all, and it grouped a command by which component *owns* its domain
  while the page grouped by what a component *accepts* and *publishes* — and the model allows those to
  differ, since a component may accept a command from a domain it does not own. Two pictures of one
  system, from two code paths, with nothing comparing them. There is now one renderer and a test that
  runs the real binary and the real generator and requires their output to match.

## [0.3.3-ess-wave-3.5] — 2026-08-20

### Added

- **A command outcome says which entity it acts on, and a transition nobody takes is refused.** An
  outcome declares `creates:`, `moves:` or `updates:`, so `CreateInvoice.accepted` creates an invoice
  and `CreateInvoice.rejected` creates nothing — the distinction lives on the outcome because a
  subject on the command would attach a state change to a refusal. A lifecycle transition no outcome
  takes is now `missing_causation`: it is a state change nothing can trigger, which is the lifecycle's
  version of a type no value can inhabit, and the refusal names the outcome that could take it.
- **The published schemas accept every spelling the parsers do.** `component:` beside `name:` in a
  specification, `id:` beside `name:` on a binding, `type:` beside `kind:` in a task, `require:` beside
  `requires:` in a workflow, and fourteen more. An editor loaded with
  `schemas/generated/ess.schema.json` marked this repository's own normative example invalid, and
  offered no fix, because the spelling it objected to was the spelling the guide's examples use. The
  aliases were always deliberate; the schema simply did not know about them, since a `#[serde(alias)]`
  is invisible to schema generation. Fifteen of the seventeen were in documents nobody had checked.
- **Conformance evidence is bound to the revision it was produced against.** A run against yesterday's
  specification no longer satisfies a requirement about today's, and a specification artifact that
  records no model digest is conformed to by nothing. The second half is deliberate and is the
  uncomfortable one: unproven is not proven, so a specification whose artifact carries no digest leaves
  its conformance requirement permanently owed until someone records one. The alternative — treating an
  unrecorded digest as "probably fine" — is how evidence outlives the thing it was evidence for.
- **`ess-conformance`** — the one piece the verification oracle cannot start without: a candidate
  command input projected into facts, and a guard decided against it. It answers with four outcomes
  rather than a boolean, because "this value does not satisfy the guard" and "this guard cannot be
  decided at all" are different answers and only the first means *try another value*. A guard ordering
  two pieces of text with no declared scale, or reading a path no type declares, is unevaluable — and
  saying so is the point, since treating it as a failure would report a specification's defect as a
  flaky test.
- **A binding that escalates must say what that emits.** `on_failure: escalate` on its own is now
  refused: write `escalate:` with `emits:` naming a declared event. "Surface it to a person" is not
  something a conformance target can be asked to prove, so a failure policy that said only that was a
  promise nobody could be held to. `retry` and `drop` are unchanged and stay single words — a retry is
  observable as another invocation, and a drop is unobservable on purpose, which is the whole reason it
  has to be typed out.
- **A property-test result carries the seed that reproduces it.** A counterexample you cannot re-run
  is a bug report without a repro, so `seed` is now part of the record — an opaque string rather than a
  number, because proptest, Hypothesis, fast-check and a fuzz corpus each spell a seed differently and
  a numeric field would force three of them to encode a lie.
- **Conformance evidence names the specification it attests**, by digest and not by a free-text string.
  A record that cannot say which specification produced it proves that some implementation passed some
  suite; it cannot prove that the implementation in front of you conforms to the specification in front
  of you.

### Changed

- **`version: 4294967296` is refused rather than silently becoming `4294967295`.** The two spellings of
  a version now agree: `v4294967296` was already refused, while the numeric form saturated, so two
  documents that disagreed about a version compared equal.
- **A YAML mapping key written twice is refused in every document this repository reads.** It was
  already refused in a specification; a protocol, principle, workflow, profile or lifecycle silently
  kept the last of the two. A profile that granted a capability twice lost one of them with no
  diagnostic.
- **A number a document cannot round-trip is refused.** `1e400` parses as an infinity, and JSON has no
  spelling for one — so it was published as `null`, turning a guard the author wrote into a guard
  nobody wrote. `.nan` likewise slipped past the constructor into a type whose documentation promises
  it cannot exist, which made ordering unreliable for every comparison against it.
- **A type or predicate nested deeper than 32 levels is refused instead of overflowing the stack.** A
  refusal names the construct and the limit; the abort it replaces named nothing.

### Fixed

- **A refused approval no longer authorises the action it refused.** A reviewer who read a change,
  refused it, and recorded that refusal was granting the production write — at three separate places in
  the engine. Also: a capability a principle denied could be downgraded to merely requiring an
  approval, an approval floor on `deployment.create:production` did not catch a profile granting the
  broader `deployment.create`, and the audit trail accepted a record that claimed a refusal and listed
  the rows it changed.
- **A validated type can no longer be conjured from a document.** Adding `Deserialize` to a type that
  is supposed to be reachable only through validation compiled and passed every check; the invariant
  every other guarantee rests on was enforced by nothing. It is now enforced mechanically.

## [0.3.2-ess-wave-3] — 2026-08-20

### Added

- **The specification now produces the documentation and the contracts, and they are in the
  repository.** [`generated/`](generated/) holds 27 files projected from
  [`examples/billing/`](examples/billing/): Markdown with Mermaid diagrams, one JSON Schema per
  command input, event payload, error payload and named type, an OpenAPI 3.1 document per component
  and an AsyncAPI 3.0 document per component. Committed rather than built on demand, because a
  contract a consumer cannot read without first installing a toolchain is a contract they copy by
  hand — and once it is committed it can be checked, so a specification change nobody regenerated
  fails the build instead of shipping a document that describes last week's system.
- **Every generated artifact says which specification produced it.** The system and its version, a
  digest of the resolved model, the compiler version and the generator version — at the top of every
  file, as a comment a person reads and as data (`x-ess-provenance`) a tool reads. When two checkouts
  disagree about an OpenAPI document the only question anyone asks is which of the two is stale, and
  the answer is now in the file rather than in whoever remembers running the generator. The digest is
  over the resolved model, not the source text, so it does not move when a comment does.
- **A named type stays a named type in every projection.** `Email` and `EmailAddress` are both a
  `String` underneath, and a projection rendering both as `{"type": "string"}` throws away the one
  distinction the model exists to make. Each keeps its own definition, its own reference and its own
  name in the schemas and in both contracts, so a code generator reading them emits two types. The
  limit is stated rather than papered over: on the wire both are a bare JSON string, so **an instance
  with the two values swapped still validates** — JSON Schema constrains structure and cannot carry
  nominal identity.
- **Where an OpenAPI path or an AsyncAPI channel comes from is a stated convention, and the generated
  document states it.** The model has no `exposures:` or `transport:` construct, so nothing in a
  specification names a method, a path, a status or a topic. Rather than invent one silently, each
  generator writes its rule into the document it produces. A command is always `POST`, at
  `/{domain wire name}/commands/{command wire name}` — `/invoices/commands/create-invoice`, with the
  `commands` segment there to stop the path pretending to be a resource, and the command's qualified
  name as the `operationId`. An event's channel address is its declared `naming.wire` or else its
  full qualified name, and every channel carries `x-ess-address-source` so a reader can tell an
  address somebody chose from one that was derived. Each of those is a rule a reviewer can disagree
  with, which is why it is written down; when `exposures:` exists it should override the convention
  rather than replace it.
- **A status code comes from the outcome, and `external` is not the caller's fault.** An outcome that
  was taken is `202`, a refusal the input decides is `422`, and a refusal decided outside the request
  is `502`. Reporting an `external` branch as a `4xx` would tell the caller to go fix the one thing
  it cannot fix and tell every retry layer in between that retrying is pointless. Outcomes sharing a
  status stay distinguishable — one response, `oneOf` the outcome schemas, each pinning its own
  `outcome` — because a status that collapsed two branches would lose the branch. `servers`,
  `security`, pagination, `201`, `ETag` and the other things an OpenAPI document usually has are
  absent: no specification backs them, and a plausible default in a contract is a claim nobody made.
- **A binding's `delivery` and `on_failure` survive the trip into the contracts.** A command some
  binding invokes with `delivery: at_least_once` gets a **required** `Idempotency-Key` header, because
  the consequence of at-least-once lands on the receiver and a surface with no way to say "this is the
  same invocation as the last one" leaves it deduplicating with no key. A command no binding invokes
  gets no header. On the messaging side both facts reach the subscriber's document, the publisher's
  document and the prose description — including `on_failure: drop`, where the work being abandoned is
  the publisher's event, so the publisher's document has to be able to say so.
- **Regenerating is byte-identical, and CI fails on a diff.** `task generate` writes the tree,
  `task generate-check` fails when the committed output is not what the specification produces, and it
  runs both inside `task check` and as a CI job of its own — "Projections up to date" — so a drifted
  contract is reported as drift rather than surfacing as an unrelated test failure. No clock, no RNG,
  `BTreeMap`/`BTreeSet` only, and a test per projection that generates twice and compares bytes.
- **A committed artifact no generator produces any more is reported as an orphan, not quietly kept.**
  A check that only compares the files a generator emits cannot see the other direction: a schema that
  was renamed or withdrawn leaves its file behind, and a consumer goes on validating against a
  contract this repository no longer stands behind. `cargo xtask generate --check` names those files
  and fails; `cargo xtask generate` removes them.
- **`protocol ess generate --kind docs|schema|openapi|asyncapi`** — and every projection at once when
  `--kind` is not given. Read-only unless `--out` is given: without it the artifacts are listed rather
  than written, because a verb that scatters files over whatever directory you happened to be in is a
  verb nobody tries twice. `--format json|yaml` carries their contents for a consumer that does not
  want a directory.
- **An entity, a view and an actor are on the generated pages.** An entity arrives with its identity
  by name and not only by type, its fields in declaration order, its invariants as the author wrote
  them, and its lifecycle as a state diagram that also lists the moves the specification does *not*
  permit — a page showing only the legal arrows reads as though the others were never considered. A
  view arrives with the entity it projects, its filter, and what its consistency level obliges a
  generated test to do: an `eventual` view asserted once races the projection, and the repair everyone
  reaches for is a sleep. An actor arrives with the commands it may invoke, drawn as edges in the
  system graph, so design §9's first arrow — somebody asking for something — is on the page instead of
  apologised for.
- **Two documents generated from one model cannot disagree about what is valid.** Every projection
  publishing a schema for a construct publishes the *same* schema for it, and a test compares them
  fragment by fragment rather than trusting that three copies of one mapping stayed equal. This
  started as a real divergence: the `AsyncAPI` document accepted an amount that was not a number and
  extra fields nobody declared, both of which the JSON Schema tree refused — so a service validating
  against one document and a service validating against the other disagreed about the same bytes. A
  difference in what a document *accepts* fails the test, and so does a difference in what it *says*
  about a construct, because a code generator reading two documents needs one answer to "which
  construct is this".
- **The published `AsyncAPI` payloads refuse what the model refuses.** They now carry
  `additionalProperties: false`, the `Decimal` pattern, the `Uuid` pattern, base64 `contentEncoding`
  for `Bytes`, `propertyNames` for a map with a non-string key, `anyOf [T, null]` for an optional
  outside a field, and a tagged `oneOf` for a union — so a branch is decidable rather than guessed. If
  you were validating events against the previous documents, messages that used to pass may now fail:
  that is the point, and each failure is something the specification never permitted.
- **An operation says which actors may invoke it** (`x-ess-may-invoke`), and no document invents a
  security scheme. `may:` states who may ask for something; an `OpenAPI` `securityScheme` states how a
  caller proves who it is, and the model says nothing about that — so a generated client would have
  implemented an authentication mechanism no specification backs.
- **A construct the documentation cannot render is named on the page where a reader went looking for
  it.** The list is an allowlist rather than a discovery, so a *new* gap fails a test and a closed one
  is a deleted entry that changes the pages with it. It is currently empty: every construct the
  specification language has reaches the IR and reaches a page. A page that quietly leaves an entity
  out reads exactly like a system that has none, which is why the empty list is a test and not a
  claim.
- **An entity, a view and an actor survive compilation.** The resolved IR carries an entity's
  identity field with its name, its fields in order, its invariants and its lifecycle; a view's source
  entity, filter, exposed fields and consistency; and an actor's grants as references that cannot name
  a command nobody declared. Before this, a specification could declare all three and everything
  downstream saw only the set of an entity's state names — so anything derived from the model was
  derived from a fraction of it.

### Not built

Test synthesis — a generated conformance suite, and an implementation deliberately wrong to prove the
suite bites — is ESS wave 4; Rust structural synthesis is wave 5. Entities, views and actors reach
the documentation but no contract projection derives from them yet: a view is a read model an
`OpenAPI` document could expose and does not, and an actor's grants are authorization rather than
authentication — the model states who may invoke a command and says nothing about how a caller proves
who it is, so no document here emits a security scheme. Every schema each document embeds is
validated against the 2020-12 meta-schema, but the `OpenAPI` and `AsyncAPI` envelopes themselves are
checked structurally rather than against the `OpenAPI` 3.1 and `AsyncAPI` 3.0 meta-schemas: neither is
vendored here.

## [0.3.1-ess-wave-2] — 2026-08-20

### Added

- **A system's decomposition, interaction and runtime shape are part of the specification.** Three
  layers above the domains, each answering something the domains cannot: which component owns which
  bounded context, what happens when an event occurs, and how many instances the design needs to be
  correct. A component is not a deployment — whether `invoice-service` ships as a process or a module
  is the topology's business, and changing that answer changes nothing in `domains/`.
- **A binding says what happens when it fails.** `delivery:` and `on_failure:` are required words, not
  defaults. A binding that can fail silently is the difference between specifying a system and
  specifying a demo, and the way that difference disappears is a default nobody read. `drop` is legal
  and has to be typed: a system that loses work is a decision, and the decision has to be findable in
  the document that made it.
- **A mapping between two bounded contexts is typechecked.** `InvoiceCreated.customer_email` into
  `SendEmail.recipient` is the one place two independently-written contexts must agree about a type,
  so it is the one place a rename in one breaks the other silently. Both sides are resolved, and the
  refusal names both paths, both types, and that no conversion is declared.
- **A type crossing must be declared, with a reason.** `Email` and `EmailAddress` are both a `String`
  underneath, and the whole value of naming them apart is that the model refuses to treat one as the
  other. `conversions:` records the crossings that are intended and requires `because:` — a conversion
  with no reason is exactly what this declaration prevents: a widening someone added to make a build
  pass, which the next reader finds and cannot evaluate. Crossings are directional.
- **`ess-compiler`** — resolution, a normalized IR whose type carries the guarantee that every
  reference resolves, and diagnostics with a stable code, a `file:line` and a machine-readable body.
  A `Specification` holds names that *probably* resolve; anything downstream either re-checked them or
  trusted that someone else had, and both are how a generator emits code for a type that does not
  exist.
- **`protocol ess compile`, `ess inspect`, `ess graph`.** `inspect` resolves a name in any of seven
  namespaces and refuses an ambiguity rather than guessing; `graph` emits DOT with components as
  clusters, and its output is byte-identical across runs.
- Generation is reproducible, and there is a test that says so rather than a comment: the same source
  compiled twice is byte-identical. `BTreeMap`/`BTreeSet` only, no clock and no RNG anywhere in the
  compiler.

### Fixed

- **A legitimate expression tree was refused.** A type reaching itself through a union was treated as
  a forbidden dependency cycle, but `Expr = union {leaf: Integer, pair: Pair}` with
  `Pair = struct {left: Expr, right: Expr}` is perfectly ordinary — every value of it bottoms out in
  a `leaf`. The rule now asks the question that matters, whether any value of the type can exist,
  rather than the shape that usually causes the answer to be no. A union needs one buildable variant,
  not all of them, and the refusal now names which requirement is unmet instead of only that
  something is.
- **A key written twice was silently discarded.** `serde_yaml` keeps the last of two identical mapping
  keys and says nothing, so a document declaring the same workload, type or even `system:` twice lost
  one of them. Reading now goes through a stage that refuses it, with the key and the line — one check
  covering every mapping in the format rather than one per section.
- A binding's mapping could not report an input mapped twice, because the raw form was a map and the
  duplicate was gone before anything could look.

### Changed

- Two new validation codes distinguish faults that were being reported as each other:
  `misspelled_reference` for text written where a reference was meant — `evnt.customer_email` parses
  clean and gets *sent* — and `unsupported_construct` for something this build will implement later, as
  against `unsupported_format_version` for a document it cannot read at all. "Upgrade the tool" and
  "write it another way" are different instructions.

## [0.3.0-ess-wave-1] — 2026-08-20

### Added

- **A system can be specified, and the specification can be refused.** `ess-domain` is the typed
  model for an Executable System Specification: domains, entities with lifecycles, commands with
  outcomes, events, errors, views with declared consistency, actors and a type system with tagged
  unions. `protocol ess validate --path <file-or-directory>` parses one and reports every problem in
  a single run, each with a code and a location.
- **[`examples/billing/`](examples/billing/)** — the single normative example, parsed by a test, and
  checked to exercise *every* construct the model has: each type kind, each primitive,
  `Optional`/`List`/`Map`, both consistency levels, an actor with grants and one without. A construct
  added to the model without reaching the example fails the build, because what the normative example
  leaves out is what nothing checks.
- **A command that can be refused says so.** Outcomes rather than a bare `emits` list: a command with
  a precondition has at least two results, and a specification recording only the happy one generates
  a suite that never checks the branch where the money does not move.
- **An outcome the input cannot decide says that too.** `external: <the cause>` marks a branch caused
  by the world — a mail provider rejecting an address — so a generator injects a fault instead of
  trying to construct an input for it. `when: false` would have claimed the branch was unreachable,
  which is a different and false statement.
- **A projection declares its consistency**, which is what decides whether a generated assertion is
  `eventually` or immediate — rather than a sleep, which makes a suite test the machine it runs on.
- **A declaration is addressable from outside** — `ep://acme/billing/ess-command/billing.invoice.CreateInvoice`,
  the protocol's own scheme rather than a new `ess://` one, so an approval against a command in a
  specification is recorded the same way as an approval against a design.
- **[`schemas/generated/ess.schema.json`](schemas/generated/ess.schema.json)** — an editor validates
  a specification as it is typed. Generated from the same Rust types the validator runs, drift-checked
  in CI, and the generated index now lists every published schema so one cannot land undocumented.
- **[`docs/guide/specification.md`](docs/guide/specification.md)** — how to write one, and what the
  model insists on.
- **[`docs/VISION.md`](docs/VISION.md)** — what this project is for, and how its two halves compose:
  AEP governs how engineering work is performed, ESS specifies what software must exist, and they
  meet at evidence.
- **[`docs/design/ess-implementor-design-v0.1.md`](docs/design/ess-implementor-design-v0.1.md)** —
  the Executable System Specification design: a system described once as a typed semantic model, from
  which contracts, documentation, tests, deployment artifacts and structural code are derived.
- **[`docs/design/ess-review-v0.1.md`](docs/design/ess-review-v0.1.md)** — a review of that design
  against what this repository learned building the same shape twice: eleven findings, three of which
  would make generated tests assert false things, and a narrower recommended v0.1 scope.
- **A task can require conformance to a specification.** `ArtifactKind::ExecutableSystemSpecification`,
  `EvidenceKind::EssConformance` and the `ess-conformance` principle — conditional on the project
  having a specification, and satisfied only by `independent: true` evidence from a
  `conformance-runner`. An agent's own report that its implementation matches the specification is
  not evidence that it does.

### Changed

- **A validation error names what actually went wrong.** A specification had been borrowing the
  protocol's document codes, so a duplicated command name reported `duplicate_principle` and a
  missing event reported `unknown_state`. Nine codes now say what they mean —
  `undeclared_reference`, `duplicate_declaration`, `missing_declaration`, `empty_declaration`,
  `conflicting_declaration`, `type_mismatch`, `unsupported_format_version`,
  `non_exhaustive_branches`, `unreachable_branch` — and sixteen places in the protocol half moved
  onto them too, so an undeclared reference is not one code in a specification and a different one
  in an artifact manifest.
- **The published schemas accept what the parser accepts.** Ten document types had a hand-written
  parser and a derived JSON Schema, so the schema described the *representation* rather than what an
  author writes: a bare `- verification` evidence requirement, a one-line objective, a
  `require_approval` capability, an `in-review` status. Twenty-eight rejections across eighteen of
  this repository's own documents. Every schema is now checked against every document the repository
  ships.
- `v01` and `ess/01` are refused. Both parsed, and both were rejected by the pattern the same build
  published — a document an editor called invalid and the tool accepted.

### Fixed

- **A schema that called the normative example invalid.** `version: v3` is what every document says;
  the published schema required an integer.
- **A guard that could not guard.** The list of validation codes the tests iterate was maintained by
  hand and had fallen five codes behind the enum, while its own comment claimed that adding a variant
  without listing it would fail the test. The enum, its wire strings and the list are now generated
  from one declaration.
- Rules that existed and were never reached: an error's payload types and an event's duplicate fields
  were checked by methods nothing called.
- A specification could name a domain in the header that nothing declares, declare an actor no domain
  owns, define two types that cannot be built without each other, filter a view on a lifecycle state
  the entity does not have, declare a type no value can be, or declare a union with no tag field. All
  six are refused.
- **A misspelt key in a type declaration was silently dropped.** `invarants:` on a value object
  parsed clean and lost the invariant, because a flattened body rules out `deny_unknown_fields` at the
  outer level. It is now a parse error with a line number.
- **A type's invariants are predicates, checked against the type's own fields**, as an entity's
  already were. `nonexistent_field >= 0` on a value object was accepted, and so was text that is not
  a predicate at all.
- A field name must survive into generated code as an identifier. `""` and `not a field name!` were
  accepted.
- An entity invariant may read the identity field. It could not, although a view projecting the same
  entity could — so a valid specification was refused with a message that was not true.
- A field may not shadow the identity's name, which produced two fields with one name and different
  types.
- A state whose only transition returns to itself is a dead end. A self-loop was counted as an exit,
  so an entity could reach a state it can never leave.
- A domain can be given a wire and display name. `naming:` on a domain file was refused, although the
  model has always carried it — so a bounded context's wire name was unreachable from any document.
- A malformed header no longer hides the reference errors under it.
- `protocol ess validate` names the file a problem is in when the specification is one file, refuses
  a directory that is not a specification instead of reading every YAML file it can find, and reads
  each file once when a symlink points back up the tree.
- `cargo xtask schema --check` fails on a schema nothing generates any more, not only on one that
  drifted.

### Not built

No compiler, no OpenAPI, no test synthesis: those are ESS waves 2 and 3 in
[`docs/plan/ess-roadmap.md`](docs/plan/ess-roadmap.md). Conformance evidence is produced by hand.

## [0.2.1] — 2026-08-20

### Added

- **A project can be discovered.** `.engineering/project.yaml` names the protocol, the profile and
  where the protocol tree lives; `protocol resolve` and `protocol evaluate` run with no arguments
  anywhere inside a project, walking up to find it. An adopting team's first command no longer needs
  four paths.
- **Project-local principles and profiles.** `.engineering/principles/` and `.engineering/profiles/`
  are merged over the protocol tree's, because no organisation's rules are entirely somebody else's.
  They are documents in the same format, validated the same way — and a project-local profile still
  cannot grant a capability the protocol's approval floor keeps behind approval.
- `protocol resolve` and `protocol evaluate` report where their inputs came from, so it is never
  ambiguous whether a flag or the project supplied them.

### Fixed

- **The approval floor was inert for every `adp/1` and `aop/1` profile.** `Protocol::extend` merged
  capabilities, evidence kinds, verifiers, phases, observables and scales — but not the approval
  floor, and neither derived protocol declares one of its own. A profile written against `adp/1`
  could therefore grant `production.write` outright and resolution would accept it, while three
  documents claimed that was impossible. The shipped profiles were unaffected because each
  hand-writes `require_approval`; the check meant to make the mistake impossible was doing nothing.
  Now inherited, with a regression test over the real documents that fails without the fix.
- **The CLI crashed when its reader stopped reading.** `protocol inspect | head -3` ended in a panic
  and a stack trace, because Rust's `println!` panics on a closed pipe. Output now ends quietly.

## [0.2.0-wave-3] — 2026-08-20

### Added

- **`aep-conformance`** — sixteen black-box suites a backend runs against itself to prove it
  implements the contract: identity, command execution, idempotency, optimistic concurrency, query,
  consistency, relations, history, immutability, audit, rejected-action audit, correlation, causation,
  provenance, events and type discovery. Reports name the *property* that failed, not the assertion,
  so a failure says what to fix.
- **Conformance levels** — `core`, `audited`, `full`. A backend states what it claims and the suite
  proves or refutes it, instead of a README asserting it.
- **`FaultyBackend`** — a wrapper that breaks exactly one property at a time. The crate's own tests
  assert that the suite responsible for each fault fails and the others still pass, because a suite
  that passes everything tells you nothing about whether it would catch anything.
- **`protocol conformance --level core|audited|full [--suite <name>] [--inject <fault>]`** — runs the
  suites, and can deliberately break a property to show which suite catches it.
- **`adp-domain`** — development types (`adp.specification/v1`, `adp.test-plan/v1`,
  `adp.acceptance-criteria/v1`, `adp.change/v1`) and commands (`adp.story.start/v1`,
  `adp.story.complete/v1`, `adp.test-plan.record/v1`, `adp.specification.satisfy/v1`). A
  specification declared satisfied by no evidence is refused — the exact claim the protocol exists to
  stop.
- **`aop-domain`** — operations types (`aop.incident/v1`, `aop.runbook/v1`, `aop.release/v1`) with
  their status ladders, and commands (`aop.incident.acknowledge|mitigate|resolve/v1`,
  `aop.release.promote|rollback/v1`). Promoting to production without naming an approval is refused
  at the command, which is a second defence beside the protocol's approval floor.
- **`docs/guide/`** — how to adopt the protocol, wire a harness to the engine, and implement and
  prove a backend.
- `Fault::caught_by()` names the suite responsible for each fault, and the crate's own tests assert
  that suite fails when the fault is injected. `DropAffected` fails eight suites, which is a finding
  about how load-bearing `affected` is rather than a flaw in the suites, and is recorded as such.

### Changed

- The in-memory backend now **refuses an update to an immutable type**. A review result records what
  someone concluded at a moment; editing it afterwards changes what the record says a person decided.
  Archiving stays available — keeping a record and editing it are different acts.

## [0.2.0-wave-2] — 2026-08-20

### Added

- **Identity.** Every addressable thing now has an opaque `EntityId`, a logical `EntityLocator`
  (`ep://acme/payments/design/passkeys-auth`), a versioned `EntityType` (`aep.design/v1`) and a
  monotonic `EntityRevision`. `AUTH-142` is a key in a locator, not identity — so two repositories can
  refer to the same design, and an approval can name the exact revision it approved.
- **`ActorRef`** — `human:alice`, `agent:planning-agent`, `service:release-controller`, `system`.
  Distinct from an evidence `Producer`: an actor bears responsibility, a producer made an observation.
  Commands carry both an actor and an executor, so "alice authorised it, agent-17 ran it" is
  answerable, and a trail that collapses them can answer neither question.
- **`aep-contract`** — the storage-independent interaction contract: `CommandService` and
  `QueryService`, command envelopes with the six identifiers that make a trail reconstructable,
  consistency tokens giving read-your-writes without sleeps, a typed failure taxonomy, and
  `TypeDescriptor` so a harness can ask what a design is instead of hard-coding it.
- **Commands** (`aep-domain::command`) — six generic (`CreateEntity`, `UpdateEntity`,
  `CreateRelation`, `RemoveRelation`, `ArchiveEntity`, `SupersedeEntity`) and three domain
  (`SubmitDesignReview`, `ApproveDesign`, `AcceptAdr`). A domain command can be validated where a
  generic patch cannot: `ApproveDesign{design@7, review}` checks that the review is about *that*
  revision.
- **Domain events** (`aep-domain::domain_event`) — a versioned event vocabulary with an open
  `Custom` variant, separate from the protocol's execution events. An event caused by a command
  names that command as its cause.
- **Audit records** (`aep-domain::audit`) — actor and executor, correlation and causation, decision
  records and change records with before/after revisions, and **rejected attempts**: a denied command
  changes nothing and still leaves a record, which is the half most systems lose.
- **`aep-backend-memory`** — a complete in-memory implementation of both contract surfaces, so the
  contract is exercised by something before anyone builds a durable backend. It passes the
  specification's nineteen-step reference scenario, including idempotent replay, stale-revision
  conflicts and the audit record a refusal leaves behind.
- **`aep-engine::trail`** — protocol decisions become audit records, and a command issued during an
  execution inherits its correlation, execution and task. A refusal by the protocol and a refusal by
  a backend now land in the same trail, queryable the same way.
- Evidence may be submitted as an entity reference, so the trail points at the stored evidence rather
  than at the engine's copy of it.
- `RelationKind::Delivers`, and `ArtifactKind::entity_type()` mapping the human-facing artifact
  vocabulary onto entity types.
- **CLI**: `protocol entity list|get|history|relations`, `protocol audit [--correlation|--entity|
  --rejected]` and `protocol describe <type>`, backed by an in-memory backend seeded from an artifact
  manifest through real commands — so seeding produces history and audit records like anything else.

### Changed

- **Nine new `ValidationCode`s** — `self_reference`, `empty_change`, `refusal_mutated_state`,
  `unreconstructable_change`, `unexplained_decision`, `redaction_inconsistent`,
  `event_payload_mismatch`, `incomplete_event_subject`, `missing_causation`. Previously these
  failures all reported `unknown_state`, so a caller could not tell "this audit record claims a
  refusal changed something" from "this workflow references a state that does not exist".
- Minimum supported Rust version is 1.85 (`Waker::noop`, which lets the contract define `async fn`
  traits without an executor dependency or a line of `unsafe`).
- A protocol may declare an **approval floor** — capabilities no profile may grant outright.
  `aep/1` declares `production.write` and `deployment.create:production`, and a profile that grants
  one fails to resolve.

## [0.2.0-wave-1] — 2026-08-20

### Added

- **The execution core.** `aep-engine` resolves a task against a document tree and answers what is
  owed, what may be done, which transitions are permitted and whether the task is complete:
  - `registry` — the documents in force, with the cross-document checks (unknown references, pinned
    version mismatches, undeclared capabilities and evidence kinds, evidence no verifier can
    establish);
  - `load` — reads a document tree, reporting every bad file with its path rather than the first;
  - `resolve` — task + registry → execution plan: `extends` chains merged, principles filtered by
    applicability, capabilities composed with the document responsible recorded for each entry,
    obligations collected, and the whole configuration checked for rules that could never fire;
  - `execution` — live state with derived facts (`evidence.first_seq.*`, `test.first_result`,
    `evidence.missing`) and a serialisable snapshot;
  - `evaluate`, `policy`, `explain` — what is owed, capability decisions naming the rule that
    decided, and the `✓ / ✗ / ?` completion checklist;
  - `engine` — the `ProtocolEngine` trait, deterministic transitions, an injected `Clock`.
- **The documents.** 42 of them: `aep/1` plus `adp/1` and `aop/1`; 21 principles across intent,
  construction, verification and governance; 4 workflows (development, incident, progressive release,
  forward-only migration); 5 profiles; 5 artifact lifecycles; artifact kind and relation definitions;
  8 templates.
- **`protocol` CLI** — `validate`, `resolve`, `inspect`, `evaluate`, `explain`, `schema`, with
  `--format text|yaml|json`.
- **Worked example** (`examples/development-passkeys/`) — a task, its artifact graph and a five-step
  evidence sequence that walks to completion, replayed by the integration tests.
- **Protocol approval floor.** A protocol may declare capabilities no profile can grant outright;
  `aep/1` declares `production.write` and `deployment.create:production`. A profile that grants one
  fails to resolve.
- **`Action::ProductionMutate`** — production changes that are not deployments now have an action, so
  a policy naming only deployments cannot let them through.
- **CI** — GitHub Actions mirroring `task check`, with schema drift as its own job.

### Fixed

- `evidence.missing` counted evidence required by conditional rules that did not apply, so a task
  could show every requirement met and still be unable to finish.
- The approval floor is now violated by any *overlap*: granting `deployment.create` for every
  environment no longer slips past a floor on `deployment.create:production`.
- A task may name the base protocol its profile refines (`aep/1` with a profile written against
  `adp/1`), which is the form the design documents use.

### Changed

- Evidence files spell the envelope's subject `about`, not `subject`, so it cannot silently consume a
  payload's own `subject` — a review's subject is the artifact reviewed.
- `protocol evaluate` exits `0` whenever it produced a report. A blocked execution is an answer, not
  a failure; `explain --action` still exits `1` when an action is refused.

## [0.1.0] — 2026-08-19

### Added

- **`aep-domain`** — the source-of-truth model: identifiers and versioned references, a three-valued
  predicate language, facts and ordered scales, capabilities with default-deny, actions, evidence with
  provenance, verifiers and counterexamples, the artifact graph with lifecycles and typed relations,
  review semantics with revision-bound approval, requirements over evidence/artifacts/reviews/
  approvals/conditions, principles with phase-timed obligations, workflows, tasks, protocols,
  profiles, execution plans and the audit event vocabulary.
- **`aep-schema`** — document reading that separates syntax from semantic failure, and JSON Schema
  generation for six document types and four interchange types.
- **`xtask schema [--check]`** — schemas are generated from the Rust types, and CI proves they match.
- Repository scaffolding: workspace, `Taskfile.yml` gate, Apache-2.0 licence, `AGENTS.md`.

[Unreleased]: https://github.com/codewandler/engineering-protocols/compare/0.4.0-ess-wave-4...HEAD
[0.4.0-ess-wave-4]: https://github.com/codewandler/engineering-protocols/compare/0.3.3-ess-wave-3.5...0.4.0-ess-wave-4
[0.3.3-ess-wave-3.5]: https://github.com/codewandler/engineering-protocols/compare/0.3.2-ess-wave-3...0.3.3-ess-wave-3.5
[0.3.2-ess-wave-3]: https://github.com/codewandler/engineering-protocols/compare/0.3.1-ess-wave-2...0.3.2-ess-wave-3
[0.3.1-ess-wave-2]: https://github.com/codewandler/engineering-protocols/compare/0.3.0-ess-wave-1...0.3.1-ess-wave-2
[0.3.0-ess-wave-1]: https://github.com/codewandler/engineering-protocols/compare/0.2.1...0.3.0-ess-wave-1
[0.2.1]: https://github.com/codewandler/engineering-protocols/compare/0.2.0-wave-3...0.2.1
[0.2.0-wave-3]: https://github.com/codewandler/engineering-protocols/compare/0.2.0-wave-2...0.2.0-wave-3
[0.2.0-wave-2]: https://github.com/codewandler/engineering-protocols/compare/0.2.0-wave-1...0.2.0-wave-2
[0.2.0-wave-1]: https://github.com/codewandler/engineering-protocols/compare/0.1.0...0.2.0-wave-1
[0.1.0]: https://github.com/codewandler/engineering-protocols/releases/tag/0.1.0
