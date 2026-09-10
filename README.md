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
| `urn:runbook:jsonld` | JSON-LD | expand / flatten / compact against a context that is itself a resource |
| `urn:runbook:selection` | Selection | typed actions + `urn:kernel:actions types=…`; `rdfs:subClassOf` reasoning |
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
| `urn:action:greet`, `urn:action:geocode`, `urn:action:mail` | `text/plain` — toy actions whose inputs are typed `schema:Person` / `schema:PostalAddress` and **required** (a missing one is a typed `MissingArgument`) | Selection |

Every graph is skolemized — the SHACL property shapes are named (`ex:AccountShape-owner`,
`<urn:runbook:shape:endpoint:title>`), never blank — and every one is a constant, cached with
no golden thread because there is nothing to cut. The Account demo lives under
`http://example.org/`, the reserved example namespace, because it is a demo of validating
*your* data; the endpoint shape is the runbook's own, under `urn:runbook:shape:*`, and is not a
vocabulary term.

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

// Mount the whole runbook into a kernel's root space.
let space = ikigai_runbook::space();

// Resolve a page as htmx HTML (the browser adapter swaps it into #runbook):
//   source urn:runbook:basics as=text/html
//
// …or as text, in the TUI:
//   source urn:runbook:basics as=text/plain
```

## Conformance

`tests/conformance.rs` runs [`ikigai-conformance`](https://github.com/ikigai-rs/ikigai-conformance)
over `space()` with **no opt-outs**: every check, every endpoint (21), clean. Declarations: the
six constant graphs are `pure` + `cacheable`; `http://example.org/` is the registered namespace
(above). What the suite cannot see is pinned by hand in the same file — the declared faces are
the faces served (both directions, from `as`'s `one_of`), the pages are live by decision, a
required toy input is required, and a shape's `sh:path` / `sh:targetClass` targets are defined
terms.

## License

Licensed under `MIT OR Apache-2.0`.
