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

## License

Licensed under `MIT OR Apache-2.0`.
