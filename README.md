# ikigai-runbook

An interactive **runbook** module for the [ikigai-core](https://crates.io/crates/ikigai-core)
resolution kernel. It exposes a set of guided, runnable demos as `urn:runbook:*` resources
and is mounted into a kernel with `space()`. The same crate is linked by **both** hosts in the
ikigai ecosystem — the native CLI's embedded space and the in-browser WebAssembly kernel — so
the runbook is authored once and runs identically in each.

## Content, not a frontend

The runbook is **content**, not an application shell. Each demo is a resource whose
*representation carries its own navigation*. In the browser it renders an
[htmx HATEOAS](https://htmx.org/examples/tabs-hateoas/) fragment — a tab strip (active tab
marked) plus a panel of `hx-get` step buttons — so switching tabs and running steps are both
just *resolving a resource*: no client-side state, no bespoke JavaScript. In the terminal the
same resource renders as text. Same resources, two adapters.

The crate only *renders*. Execution lives in the host: one small adapter maps an `hx-get`'s
`/k/<command>` to `engine.eval`. This crate emits no `unsafe` code and pulls in only
`ikigai-core` plus `serde`/`serde_json`.

## Content negotiation

Every page content-negotiates on the `as` argument:

| `as` value | Rendering | Consumer |
| --- | --- | --- |
| `text/html` (default) | htmx HATEOAS fragment — tab strip + `hx-get` step buttons | browser kernel |
| `text/plain` | the tab list, intro, and steps as a numbered runnable list | TUI |
| `application/json` | `{ id, label, intro, steps: [{ label, cmd, note }] }` | TUI (run a step by number) |

All three are declared: the `as` input is typed (`xsd:string`), its `one_of` is exactly this
list (`ikigai_runbook::PAGE_FACES`, default `text/html`), and each is a declared output — so a
consumer reading the manifold sees every face, and `urn:kernel:validate` refuses an `as` value
the renderer would only have fallen back from. A page is served **live** on purpose: the tab
strip it carries is host state (`add_tab` / `hide_tab`) that no golden thread tracks, so a cached
page would keep offering a tab the host has withdrawn.

## Built-in tabs

Each tab is bound as `urn:runbook:<id>` (`source` + `meta`). Adding a demo is adding an entry
to the in-crate table — no per-host code, in either frontend.

| Resource | Tab | What it demonstrates |
| --- | --- | --- |
| `urn:runbook:basics` | Basics | resolving a resource by name; functions as resources; `\|` piping |
| `urn:runbook:piping` | Piping | `\|` pipe, `..` map-over-items, `( a ; b )` fork — concurrent under a pool |
| `urn:runbook:http` | HTTP | `urn:httpGet` resolving a URL as a resource, header-cached |
| `urn:runbook:constraints` | Constraints | `urn:kernel:constraint` / `urn:kernel:scheduler` — Goldratt "find the constraint" |
| `urn:runbook:zerotrust` | ZeroTrust | `cap` narrowing: writes refused, reads resolve, jail + network gating |
| `urn:runbook:linkeddata` | Linked Data | `urn:rdf:transrept` to Turtle; cacheability flowing down the pipe |
| `urn:runbook:transrept` | Transreption | one graph, many syntaxes — N-Triples, RDF/XML, JSON-LD, an HTML table |
| `urn:runbook:sniff` | Sniff & dispatch | `urn:sniff` types opaque bytes; `urn:transrept:auto` routes to the transreptor |
| `urn:runbook:bytes` | Bytes | bytes that are not text; gzip and gunzip through `ikigai-compress`'s transreptors; `as` naming what gzip does not record; a `max-bytes` bound that refuses past its limit |
| `urn:runbook:jsonld` | JSON-LD | expand / flatten / compact against a context that is itself a resource |
| `urn:runbook:selection` | Selection | typed actions + `urn:kernel:actions types=…`; `rdfs:subClassOf` reasoning through the host's built map, then the same question through the OWL 2 RL reasoner (`urn:reason:inferred`); an edit to the alignment that the reasoner follows and the built map does not, with the measured cost of each |
| `urn:runbook:vocabularies` | Two vocabularies | `owl:equivalentClass` and `rdfs:subPropertyOf` between schema.org and FOAF terms: one SPARQL query over the closure finds people written in either |
| `urn:runbook:typing` | Domain & range | an entity with no `rdf:type` typed by `rdfs:domain` / `rdfs:range`, then `urn:kernel:actions` selecting for those types |
| `urn:runbook:topics` | Topics | a SKOS scheme; `skos:broaderTransitive` from the closure narrows tagged posts to everything under one topic |
| `urn:runbook:thread` | The thread | edit one axiom: the cache tally (`2 cached · 1 computed`) shows only the closure that read it recomputing |
| `urn:runbook:bounds` | Reasoner bounds | a closure past the reasoner's output bound, and an input past its input bound, refused as OverBudget, never truncated |
| `urn:runbook:shacl` | SHACL | validate good and bad data against shapes; the kernel validates its own catalog |
| `urn:runbook:lisp` | Lisp | `urn:lisp:eval`, verbs as functions, s-expr → RDF, signing, generated aliases |

The demos' data travel with the module, as resources of their own — bound so every host runs
the same demo with no host-specific data:

| Resource | Serves | Used by |
| --- | --- | --- |
| `urn:data:ik-context` | `application/ld+json` — a context whose `@vocab` is the ikigai namespace | JSON-LD `compact` |
| `urn:data:alignment` | `text/turtle` — `foaf:Person rdfs:subClassOf schema:Person` | Selection (the host folds it into the subclass closure) |
| `urn:data:account-shape`, `urn:data:account-ok`, `urn:data:account-bad` | `text/turtle` — an Account shape and a conforming / violating instance, under `http://example.org/` | SHACL |
| `urn:data:endpoint-shape` | `text/turtle` — `<urn:runbook:shape:endpoint>`, targeting `ik:Endpoint` / `ik:title` | SHACL (dogfood) |
| `urn:data:alignment-edit`, `urn:data:people` | `text/turtle` — the alignment plus `ex:Customer rdfs:subClassOf foaf:Person`; `ex:ada a foaf:Person`, `ex:bob a ex:Customer` | Selection (the edit), The thread |
| `urn:data:vocab-alignment`, `urn:data:people-mixed` | `text/turtle` — `foaf:Person owl:equivalentClass schema:Person`, `foaf:name rdfs:subPropertyOf schema:name`; one person in each vocabulary | Two vocabularies |
| `urn:data:typing` | `text/turtle` — `schema:address` with a domain and a range, and an untyped `ex:carol` | Domain & range |
| `urn:data:topics`, `urn:data:tagged` | `text/turtle` — a SKOS scheme (with SKOS's `broader ⊑ broaderTransitive` and transitivity axioms) and four posts tagged with its topics | Topics, The thread |
| `urn:data:thread-graphs` | `text/plain` — the two graph IRIs the Thread page reasons over, one a line | The thread |
| `urn:data:fan-small`, `urn:data:fan-large`, `urn:data:fan-over-input` | `text/turtle` — generated: one class under 3 / 150 / 150 superclasses with 4 / 840 / 851 members | Reasoner bounds |
| `urn:action:greet`, `urn:action:geocode`, `urn:action:mail` | `text/plain` — toy actions whose inputs are typed `schema:Person` / `schema:PostalAddress` and **required** (a missing one is a typed `MissingArgument`) | Selection |

Every graph is skolemized — the SHACL property shapes are named (`ex:AccountShape-owner`,
`<urn:runbook:shape:endpoint:title>`), never blank — and every one is a constant, cached with
no golden thread because there is nothing to cut. The Account demo lives under
`http://example.org/`, the reserved example namespace, because it is a demo of validating
*your* data; the endpoint shape is the runbook's own, under `urn:runbook:shape:*`, and is not a
vocabulary term.

The **OWL 2 RL** steps (the end of Selection, and the five pages after it) need the
reasoner in the host's kernel: `ikigai-reason` (`urn:reason:closure`, `urn:reason:inferred`;
this crate tests against 0.3.3, its first published release) beside `ikigai-rdf`. Two vocabularies and Topics also
query with `ikigai-sparql`, and the edits write files through `ikigai-fs` (`urn:file:*`), so a
reasoner answer follows an edit only when the host's file mount is cacheable and the write goes
through the kernel, which cuts the file's thread. `urn:sparql:select` reads its graphs by IRI,
not from a pipe, so those two pages write the closure to a file and query that: a snapshot, run
again after an edit. A host without the reasoner should `hide_tab` those five pages (Selection's
first six steps need only the kernel). `tests/reasoning_demos.rs` runs every step of the six
pages, in order, through the real REPL engine over those modules, with the host's built
subclass map, and prints a transcript under `cargo test -- --nocapture`.

The notes quote the measurements in `ikigai-reason`'s README ("Choosing: construction, a
property path, or a reasoner", ledger #2640; native, release build). On raw time the built
map wins in every measured graph (a rebuild after an edit is 2.6 to 13 times faster than the
reasoner's recompute); the reasoner wins on coverage (`equivalentClass`, which the built map
silently misses, `subPropertyOf`, domain/range, `inverseOf`) and on edits (a cut, no rebuild).
The hybrid, reason once and build the map from the closure, needs a kernel rebuild that no step
can perform, so Selection describes it in a note rather than running it. OWL RL also adds
`x a owl:Thing` for every class and individual (and `owl:Thing a owl:Class`, …); the notes say
which lines those are wherever `urn:reason:inferred` shows them.

The **Bytes** page needs two modules in the host's kernel: `ikigai-compress` (`urn:compress:gzip`,
`urn:decompress:gzip`) and `ikigai-sniff` (`urn:sniff`, `urn:transrept:auto`). Both build for
wasm. A host that does not link `ikigai-compress` should `hide_tab("bytes")`, the same as the Lisp
tab below. `tests/bytes_demo.rs` runs every step of the page through the real REPL engine over
those modules, so a step that stops doing what its note says is a red test.

## Host-extensible tabs — `add_tab` and `hide_tab`

A host shapes the strip in both directions: it can append a tab the shared module doesn't know
about, and it can withdraw one it cannot serve.

```rust
ikigai_runbook::add_tab("identity", "Identity"); // web-demo's browser-only tab
ikigai_runbook::hide_tab("lisp");                // …which has no urn:lisp:eval to run
```

`add_tab(id, label)` is idempotent (a host may build its kernel more than once). The host also
binds `urn:runbook:<id>` itself; that endpoint's `text/html` representation should lead with
`render_tab_strip(<id>)` so the strip stays identical across every tab — the native CLI simply
never registers any extras.

`hide_tab(id)` is its mirror: the id stops appearing in every strip this crate renders, HTML
and text alike. It exists because a built-in page can be unservable in a particular host — the
in-browser WASM kernel does not link `ikigai-lisp` (Steel doesn't go to wasm), so its **Lisp**
tab offered steps that could only answer `no endpoint resolved for urn:lisp:eval`. Three
behaviours are pinned by tests:

| Case | Behaviour |
| --- | --- |
| unknown id | accepted silently — so `hide_tab` and `add_tab` **commute** (either order gives the same strip). The cost: a typo hides nothing and says nothing. |
| the hidden tab is **active** | the strip omits it and marks nothing selected — a tab for an unservable page is not put back in front of the user who reached it |
| resolving a hidden page | still works. Hiding is presentation, not unbinding: `space()` binds every built-in regardless, so the kernel's contents never depend on whether `hide_tab` ran before or after it |

`tab_ids()` returns the ids the strip would render (built-ins + `add_tab`s − `hide_tab`s) — the
seam for a host that wants a mistyped `hide_tab` to be loud.

**The limitation, stated rather than hidden:** `hide_tab` requires the host to *know* what it
cannot serve. It fixes the Lisp tab; it does not close the class, and the next tab a host can't
bind regresses the same way. The honest version — a strip that probes the kernel and omits
whatever does not resolve — was considered and deferred.

## Usage

```rust
use ikigai_core::Kernel; // or however the host assembles its root space

// Mount the whole runbook into a kernel's root space. It names itself
// `urn:iki:space:runbook` (`ikigai_runbook::SPACE_ID`).
let space = ikigai_runbook::space();

// Resolve a page as htmx HTML (the browser adapter swaps it into #runbook):
//   source urn:runbook:basics as=text/html
//
// …or as text, in the TUI:
//   source urn:runbook:basics as=text/plain
```

## Conformance

`tests/conformance.rs` runs [`ikigai-conformance`](https://github.com/ikigai-rs/ikigai-conformance)
over `space()` with **no opt-outs**: every check, every endpoint (39), clean. Declarations: the
seventeen constant graphs are `pure` + `cacheable`; `http://example.org/` is the registered namespace
(above); `space()` is self-named `urn:iki:space:runbook` (`SPACE_ID`). What the suite cannot see is pinned by hand in the same file — the declared faces are
the faces served (both directions, from `as`'s `one_of`), the pages are live by decision, a
required toy input is required, and a shape's `sh:path` / `sh:targetClass` targets are defined
terms.

## Accessibility

The pages target WCAG 2.2 AA for the markup and text this crate emits, and make no conformance
claim. [ACCESSIBILITY.md](ACCESSIBILITY.md) says what has been checked (one axe run over every
page's HTML), what has not (no screen reader, no host's styling, no check in CI), and the barriers
known today. Report a barrier with an issue labeled `accessibility`.

## License

Licensed under `MIT OR Apache-2.0`.
