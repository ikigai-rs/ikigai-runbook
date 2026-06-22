//! Interactive runbook for ikigai — guided, runnable demos as `urn:runbook:*`
//! resources.
//!
//! The runbook is **content, not a frontend**. Each demo is a resource whose
//! representation carries its own navigation: in the browser it renders an htmx
//! [HATEOAS](https://htmx.org/examples/tabs-hateoas/) fragment — the tab strip (with
//! the active tab marked) plus a panel of `hx-get` step buttons — so switching tabs
//! and running steps are both just *resolving a resource*, with no client-side state
//! and no bespoke JavaScript. In the terminal the same resource renders as text.
//!
//! Two hosts link this one module — the CLI's embedded space and the in-browser
//! WASM kernel — so the runbook is authored once and runs in both. Execution lives in
//! the host (one small adapter turns an `hx-get`'s command into `engine.eval`); this
//! crate only *renders*. Content-negotiates on the `as` argument: `text/html`
//! (default, htmx) or `text/plain` (the TUI).

#![forbid(unsafe_code)]

use ikigai_core::{
    ArgSpec, Description, EndpointSpace, Exact, FnEndpoint, Invocation, ReprType, Representation,
    Result, Verb,
};

/// One runnable step within a demo: a button label, the REPL command it runs, and a
/// one-line note on what to observe.
#[derive(serde::Serialize)]
struct Step {
    label: &'static str,
    cmd: &'static str,
    note: &'static str,
}

/// A runbook page: an id (→ `urn:runbook:<id>`), a tab label, intro prose, and steps.
#[derive(serde::Serialize)]
struct Demo {
    id: &'static str,
    label: &'static str,
    intro: &'static str,
    steps: &'static [Step],
}

/// The runbook's pages, in tab order. Adding a demo is adding an entry here — no
/// per-host code, in either frontend.
static DEMOS: &[Demo] = &[
    Demo {
        id: "basics",
        label: "Basics",
        intro: "A resource is resolved by name; functions are resources too, and `|` \
                pipes one resolution's output into the next. The same engine drives this \
                page, the terminal, and the desktop CLI.",
        steps: &[
            Step {
                label: "uppercase",
                cmd: "source urn:fn:toUpper hello",
                note: "a function resource",
            },
            Step {
                label: "pipe",
                cmd: "source urn:fn:toUpper hi | urn:demo:wrap",
                note: "pipe output into the next stage",
            },
            Step {
                label: "host info",
                cmd: "source urn:host:info",
                note: "the host names itself (uncacheable — a live fact)",
            },
        ],
    },
    Demo {
        id: "piping",
        label: "Piping",
        intro: "`|` pipes a resolution's output into the next; `..` maps a stage over \
                each newline item and rejoins; `( a ; b )` forks the same input to several \
                branches. Under a thread pool the map and fork branches run concurrently.",
        steps: &[
            Step {
                label: "pipe",
                cmd: "source urn:fn:toUpper hi | urn:demo:wrap",
                note: "one stage's output feeds the next",
            },
            Step {
                label: "map",
                cmd: "source urn:demo:split a,b,c .. urn:fn:toUpper",
                note: "run the stage per newline item, rejoin",
            },
            Step {
                label: "fork",
                cmd: "source urn:demo:split x,y,z | ( urn:fn:toUpper ; urn:fn:reverseList )",
                note: "fan the input to each branch, join the outputs",
            },
        ],
    },
    Demo {
        id: "http",
        label: "HTTP",
        intro: "A URL is a resource — `urn:httpGet` resolves it through the kernel, cached \
                by the origin's headers. In the browser, fetch reaches only CORS-enabled \
                https origins (the native CLI has no such limit).",
        steps: &[
            Step {
                label: "fetch JSON",
                cmd: "source urn:httpGet url=https://httpbin.org/uuid",
                note: "a live GET, resolved in WebAssembly",
            },
            Step {
                label: "a FOAF profile",
                cmd: "source urn:httpGet url=https://w3id.org/people/bsletten",
                note: "a persistent identifier → RDF, https + CORS the whole way",
            },
        ],
    },
    Demo {
        id: "constraints",
        label: "Constraints",
        intro: "The kernel keeps a rolling record of where uncached compute goes. Do some \
                work, then ask where the throughput constraint is — Goldratt's \"identify \
                the constraint,\" answered by the kernel.",
        steps: &[
            Step {
                label: "do some work",
                cmd: "source urn:fn:compose src=urn:data:page",
                note: "compose the page — fans out several markers",
            },
            Step {
                label: "where's the bottleneck?",
                cmd: "source urn:kernel:constraint",
                note: "heaviest uncached resource first",
            },
            Step {
                label: "the scheduler",
                cmd: "source urn:kernel:scheduler",
                note: "backend and live task counts",
            },
        ],
    },
    Demo {
        id: "zerotrust",
        label: "ZeroTrust",
        intro: "The session starts at root authority, so the first write lands. Narrow \
                the capability and watch a write get refused — while reads still resolve, \
                and the jail refuses to escape its root even at full authority. The same \
                model gates the network: grant one host and a fetch to anywhere else is \
                refused before it leaves. Same enforcement as the native CLI, in WebAssembly.",
        steps: &[
            Step {
                label: "1 · write a file",
                cmd: "sink urn:file:note.txt remember the milk",
                note: "root session — the write lands",
            },
            Step {
                label: "2 · cap read-only",
                cmd: "cap read-only",
                note: "voluntarily give up authority; it can only shrink",
            },
            Step {
                label: "3 · write → denied",
                cmd: "sink urn:file:note.txt nope",
                note: "refused — the capability grants read, not write",
            },
            Step {
                label: "4 · read → ok",
                cmd: "source urn:file:note.txt",
                note: "reads still resolve under the narrowed capability",
            },
            Step {
                label: "5 · escape jail → denied",
                cmd: "source urn:file:../../etc/hosts",
                note: "the jail refuses `..` even at root — the floor beneath the capability",
            },
            Step {
                label: "6 · cap reset",
                cmd: "cap reset",
                note: "back to root identity",
            },
            Step {
                label: "7 · grant one host",
                cmd: "cap urn:cap:net:httpbin.org",
                note: "hand an agent the web, narrowly — only httpbin.org",
            },
            Step {
                label: "8 · fetch it → ok",
                cmd: "source urn:httpGet url=https://httpbin.org/uuid",
                note: "allowed — the URL's host is within the grant",
            },
            Step {
                label: "9 · fetch elsewhere → denied",
                cmd: "source urn:httpGet url=https://w3id.org/people/bsletten",
                note: "refused — w3id.org isn't in the grant; it resolves fine at full authority, \
                       so the capability is the gate, not reachability",
            },
            Step {
                label: "10 · cap reset",
                cmd: "cap reset",
                note: "back to root identity",
            },
        ],
    },
];

/// The runbook space: binds `urn:runbook:<id>` for every [`Demo`]. Mount it in any
/// kernel's root (the CLI's embedded space, the in-browser kernel) and the whole
/// runbook is available, identically.
pub fn space() -> EndpointSpace {
    let mut space = EndpointSpace::new();
    for demo in DEMOS {
        space = space.bind(
            Exact::new(format!("urn:runbook:{}", demo.id)),
            FnEndpoint::new(
                format!("runbook-{}", demo.id),
                move |inv: &Invocation<'_>| render(demo, inv),
            )
            .with_description(
                Description::new(format!("runbook-{}", demo.id))
                    .title(demo.label)
                    .summary("A runbook page — guided, runnable steps.")
                    .verb(Verb::Source)
                    .verb(Verb::Meta)
                    .input(ArgSpec::new("as").summary(
                        "representation: text/html (default, htmx), text/plain, or \
                             application/json (structured, for the TUI)",
                    ))
                    .output("text/html;charset=utf-8"),
            ),
        );
    }
    space
}

/// Render `demo` per the requested `as` type — `text/plain` for the terminal, htmx
/// HTML otherwise.
fn render(demo: &Demo, inv: &Invocation<'_>) -> Result<Representation> {
    let as_type = inv.inline_str("as").unwrap_or("text/html");
    if as_type.starts_with("application/json") {
        // Structured form: `{ id, label, intro, steps: [{ label, cmd, note }] }` — the
        // TUI sources this to render the page and run a step by its number.
        let json = serde_json::to_string(demo)
            .map_err(|e| ikigai_core::Error::Endpoint(format!("runbook json: {e}")))?;
        Ok(repr("application/json", json))
    } else if as_type.starts_with("text/plain") {
        Ok(repr("text/plain", render_text(demo)))
    } else {
        Ok(repr("text/html", render_html(demo)))
    }
}

/// Minimal HTML escaping for command strings embedded in `hx-get` attributes and
/// `<code>` — commands carry `"`, `|`, `<`, `&` that would otherwise break the markup.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn repr(media: &str, body: String) -> Representation {
    Representation::new(
        ReprType::new(media).with_param("charset", "utf-8"),
        body.into_bytes(),
    )
}

/// The htmx HATEOAS fragment: the tab strip (active tab marked) followed by the
/// active demo's panel. Switching tabs `hx-get`s another `urn:runbook:*` into the
/// `#runbook` container; running a step `hx-get`s its command into `#rb-out`. The
/// host adapter maps `/k/<command>` → `engine.eval`. No client-side state.
fn render_html(active: &Demo) -> String {
    let mut tabs = String::from("<nav class=\"rb-tabs\" role=\"tablist\">");
    for demo in DEMOS {
        let selected = demo.id == active.id;
        tabs.push_str(&format!(
            "<button role=\"tab\" class=\"rb-tab{cls}\" aria-selected=\"{sel}\" \
             hx-get=\"/k/source urn:runbook:{id} as=text/html\" \
             hx-target=\"#runbook\" hx-swap=\"innerHTML\">{label}</button>",
            cls = if selected { " selected" } else { "" },
            sel = selected,
            id = demo.id,
            label = demo.label,
        ));
    }
    tabs.push_str("</nav>");

    let mut steps = String::from("<ol class=\"rb-steps\">");
    for step in active.steps {
        let cmd = esc(step.cmd);
        steps.push_str(&format!(
            "<li><button class=\"rb-step\" hx-get=\"/k/{cmd}\" hx-target=\"#rb-out\" \
             hx-swap=\"beforeend\">{label}</button> <code class=\"rb-cmd\">{cmd}</code>\
             <span class=\"rb-note\">{note}</span></li>",
            label = step.label,
            note = step.note,
        ));
    }
    steps.push_str("</ol>");

    format!(
        "{tabs}<section class=\"rb-panel\" role=\"tabpanel\">\
         <p class=\"rb-intro\">{intro}</p>{steps}\
         <pre id=\"rb-out\" class=\"rb-out\" aria-live=\"polite\"></pre></section>",
        intro = active.intro,
    )
}

/// The terminal rendering: the tab list, the intro, and the steps as a numbered,
/// runnable list. (The TUI runs a step by issuing its command; it can't run htmx.)
fn render_text(active: &Demo) -> String {
    let mut out = String::new();
    let tabs: Vec<String> = DEMOS
        .iter()
        .map(|d| {
            if d.id == active.id {
                format!("[{}]", d.label)
            } else {
                d.label.to_string()
            }
        })
        .collect();
    out.push_str(&format!("runbook · {}\n", tabs.join("  ")));
    out.push_str(&format!("\n{}\n\nsteps:\n", active.intro));
    for (i, step) in active.steps.iter().enumerate() {
        out.push_str(&format!(
            "  {}. {}\n     {}\n     — {}\n",
            i + 1,
            step.label,
            step.cmd,
            step.note,
        ));
    }
    out
}
