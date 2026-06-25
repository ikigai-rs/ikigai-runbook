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

## Host-extensible tabs

A host can append its own tabs to the strip:

```rust
ikigai_runbook::add_tab("identity", "Identity"); // web-demo's browser-only tab
```

`add_tab(id, label)` is idempotent (a host may build its kernel more than once). The host also
binds `urn:runbook:<id>` itself; that endpoint's `text/html` representation should lead with
`render_tab_strip(<id>)` so the strip stays identical across every tab — the native CLI simply
never registers any extras.

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

## License

Licensed under `MIT OR Apache-2.0`.
