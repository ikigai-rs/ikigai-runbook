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
//! (default, htmx), `text/plain` (the TUI) or `application/json` (structured); any
//! other `as` is refused with `InvalidArgument` ([`PAGE_FACES`]).
//!
//! A host shapes the strip in both directions: [`add_tab`] appends a page the shared
//! module doesn't know about, and [`hide_tab`] withdraws one this host cannot serve.

#![forbid(unsafe_code)]

use ikigai_core::{
    space_iri, ArgSpec, Description, EndpointSpace, Error, Exact, FnEndpoint, Invocation, ReprType,
    Representation, Result, Verb,
};
use ikigai_fn::html::{attr, html_text};

/// The name [`space`] claims: `urn:iki:space:runbook`.
pub const SPACE_ID: &str = "urn:iki:space:runbook";

/// The datatype of a by-value scalar input, as the manifold declares it.
const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";

/// The faces every runbook page serves, in the order `as` accepts them: the first is
/// the default. This list IS the page's declared outputs and the `as` input's `one_of`,
/// so the manifold and the renderer cannot drift apart: `urn:kernel:validate` refuses an
/// `as` outside it before dispatch, and the renderer refuses the same value after.
pub const PAGE_FACES: [&str; 3] = ["text/html", "text/plain", "application/json"];

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
                pipes one resolution's output into the next — a resource can even branch on \
                the value of another. The same engine drives this page, the terminal, and \
                the desktop CLI.",
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
            Step {
                label: "branch (lazy)",
                cmd: "source urn:fn:conditional if=urn:demo:echo/true then=urn:demo:echo/taken else=urn:does:not:exist",
                note: "sources `if`, then only the taken branch — the missing `else` is never \
                       touched (conditional is the lazy sibling of compose). Flip `true`→`false` \
                       to watch it reach for the branch that isn't there.",
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
    Demo {
        id: "linkeddata",
        label: "Linked Data",
        intro: "Transreption rewrites RDF from one syntax to another — here to Turtle — parsed \
                and re-serialized through the kernel. A result is only as cacheable as its \
                source: the kernel's own `urn:kernel:catalog` (every bound endpoint described \
                as RDF) is stable, so re-resolving the pipeline hits the cache; a live web fetch \
                with no `Cache-Control` never does. Cacheability flows down the pipe — the \
                transform inherits its source's. Run each twice and watch the tag.",
        steps: &[
            Step {
                label: "catalog → Turtle",
                cmd: "source urn:kernel:catalog | urn:rdf:transrept as=text/turtle",
                note: "the kernel describes itself; cacheable — re-run shows [cached]",
            },
            Step {
                label: "my FOAF → Turtle",
                cmd: "source urn:httpGet url=https://w3id.org/people/bsletten | urn:rdf:transrept as=text/turtle",
                note: "a live fetch with no Cache-Control → [uncacheable] every time",
            },
        ],
    },
    Demo {
        id: "transrept",
        label: "Transreption",
        intro: "One resource, many representations. `urn:rdf:transrept` is a first-class \
                ik:Transreptor — it declares the media types it converts between (its \
                from/to matrix) and re-serializes the same graph into any of them. Here the \
                kernel's own catalog goes out as N-Triples, RDF/XML, JSON-LD, and a \
                human-readable HTML table — same triples, different syntax. Versioning a \
                payload is choosing a representation, not a new identity.",
        steps: &[
            Step {
                label: "→ N-Triples",
                cmd: "source urn:kernel:catalog | urn:rdf:transrept as=application/n-triples",
                note: "one fully-qualified triple per line",
            },
            Step {
                label: "→ RDF/XML",
                cmd: "source urn:kernel:catalog | urn:rdf:transrept as=application/rdf+xml",
                note: "the same graph, XML syntax",
            },
            Step {
                label: "→ JSON-LD",
                cmd: "source urn:kernel:catalog | urn:rdf:transrept as=application/ld+json",
                note: "RDF as idiomatic JSON",
            },
            Step {
                label: "→ HTML table",
                cmd: "source urn:kernel:catalog | urn:rdf:transrept as=text/html",
                note: "the human view — subject / predicate / object",
            },
        ],
    },
    Demo {
        id: "sniff",
        label: "Sniff & dispatch",
        intro: "Opaque bytes — a fetch with a missing Content-Type, a file, a pasted blob — \
                carry no type. `urn:sniff` detects the concrete media type from the bytes; \
                `urn:transrept:auto` then sniffs *and* routes to the matching transreptor, so \
                you transrept without naming the input type. When nothing can reach the \
                target it refuses cleanly, naming the sniffed type, rather than feeding bytes \
                to the wrong parser.",
        steps: &[
            Step {
                label: "what is this?",
                cmd: "source urn:kernel:catalog | urn:sniff",
                note: "classifies the opaque bytes → text/turtle",
            },
            Step {
                label: "auto → HTML",
                cmd: "source urn:kernel:catalog | urn:transrept:auto as=text/html",
                note: "sniffed turtle, selected the RDF transreptor, ran it — no input type given",
            },
            Step {
                label: "auto → RDF/XML",
                cmd: "source urn:kernel:catalog | urn:transrept:auto as=application/rdf+xml",
                note: "same dispatch, a different target representation",
            },
            Step {
                label: "no path → refused",
                cmd: "source urn:kernel:catalog | urn:transrept:auto as=application/pdf",
                note: "nothing converts turtle → pdf, so it refuses (naming the sniffed type)",
            },
        ],
    },
    Demo {
        id: "bytes",
        label: "Bytes",
        intro: "Not every representation is text. `urn:compress:gzip` turns its input into \
                `application/gzip` bytes, and `urn:decompress:gzip` turns them back. Both are \
                registered transreptors, so the kernel can plan through gunzip without anyone \
                naming it. A gzip stream records no media type, so whoever gunzips it says \
                what the bytes are with `as` (the default is `application/octet-stream`). And \
                because a few hundred bytes of gzip can expand to gigabytes, gunzip stops at \
                `max-bytes`: past the limit it refuses, naming the limit, rather than handing \
                back a truncated answer that looks whole.",
        steps: &[
            Step {
                label: "gzip → not text",
                cmd: "source urn:compress:gzip hello hello hello",
                note: "the output is gzip bytes, not UTF-8, so it is refused here with its \
                       size rather than printed as garbage; a `sink` would store it as it is",
            },
            Step {
                label: "what are these bytes?",
                cmd: "source urn:compress:gzip hello hello hello | urn:sniff",
                note: "application/gzip, read from the stream's first two bytes",
            },
            Step {
                label: "gzip, then gunzip",
                cmd: "source urn:compress:gzip hello hello hello \
                      | urn:decompress:gzip as=text/plain",
                note: "the same 17 bytes back; `as` says they are text/plain, because gzip \
                       does not record a type",
            },
            Step {
                label: "gunzip, unnamed",
                cmd: "source urn:compress:gzip hello hello hello | urn:transrept:auto as=text/plain",
                note: "the input is gzip, not text: asked only for text/plain, \
                       `urn:transrept:auto` sniffs application/gzip and the kernel finds the \
                       route through `urn:decompress:gzip` itself; nobody named gunzip",
            },
            Step {
                label: "at the bound → ok",
                cmd: "source urn:compress:gzip hello hello hello \
                      | urn:decompress:gzip max-bytes=17 as=text/plain",
                note: "17 bytes out, a limit of 17: allowed",
            },
            Step {
                label: "past the bound → refused",
                cmd: "source urn:compress:gzip hello hello hello \
                      | urn:decompress:gzip max-bytes=16 as=text/plain",
                note: "refused, naming the 16-byte limit: no truncated answer that looks whole",
            },
        ],
    },
    Demo {
        id: "jsonld",
        label: "JSON-LD",
        intro: "JSON-LD is RDF that reads as ordinary JSON. The same catalog graph runs \
                through three operators — each a lazily-loaded `urn:jsonld:*` module \
                endpoint. **expand** drops the context and writes every property as a full \
                IRI; **flatten** hoists every node to the top level, keyed by `@id`; \
                **compact** shortens an expanded graph against a context (`@vocab` here maps \
                the ikigai namespace, so properties come back as bare terms) — the seed of \
                the trust-boundary egress filter.",
        steps: &[
            Step {
                label: "→ JSON-LD",
                cmd: "source urn:kernel:catalog | urn:rdf:transrept as=application/ld+json",
                note: "the catalog graph as idiomatic JSON-LD",
            },
            Step {
                label: "expand",
                cmd: "source urn:kernel:catalog | urn:rdf:transrept as=application/ld+json \
                      | urn:jsonld:expand",
                note: "no context — every property becomes a full IRI",
            },
            Step {
                label: "flatten",
                cmd: "source urn:kernel:catalog | urn:rdf:transrept as=application/ld+json \
                      | urn:jsonld:flatten",
                note: "every node hoisted to the top level, keyed by @id",
            },
            Step {
                label: "compact",
                cmd: "source urn:kernel:catalog | urn:rdf:transrept as=application/ld+json \
                      | urn:jsonld:expand \
                      | urn:jsonld:compact context=urn:data:ik-context",
                note: "shorten the expanded graph against a context — itself a resource",
            },
        ],
    },
    Demo {
        id: "selection",
        label: "Selection",
        intro: "Endpoints declare the RDF *type* each input needs. `urn:kernel:actions \
                types=…` then answers \"given entities of these types, what can I do?\" — it \
                returns exactly the endpoints whose required typed inputs are all satisfied. \
                Tool-selection done by the kernel: a deterministic query over the catalog, \
                not a guess (an agent would let an LLM disambiguate only what's left). Three \
                toy actions are mounted — **greet** needs a Person, **geocode** needs a \
                PostalAddress, **mail** needs both. And it *reasons*: the host loads an \
                alignment graph (`urn:data:alignment`) with `foaf:Person rdfs:subClassOf \
                schema:Person`, so a `foaf:Person` entity satisfies the `schema:Person` \
                action too — add one triple, gain an affordance.",
        steps: &[
            Step {
                label: "have a Person",
                cmd: "source urn:kernel:actions types=https://schema.org/Person",
                note: "only actions whose required typed inputs are all satisfied → greet",
            },
            Step {
                label: "have an Address",
                cmd: "source urn:kernel:actions types=https://schema.org/PostalAddress",
                note: "→ geocode (mail still needs a Person too)",
            },
            Step {
                label: "have both",
                cmd: "source urn:kernel:actions \
                      types=https://schema.org/Person,https://schema.org/PostalAddress",
                note: "mail unlocks as well — more types, more affordances",
            },
            Step {
                label: "run a selected action",
                cmd: "source urn:action:greet who=Ada",
                note: "the type is metadata for selection; invoke the action like any endpoint",
            },
            Step {
                label: "the alignment graph",
                cmd: "source urn:data:alignment",
                note: "one triple: foaf:Person rdfs:subClassOf schema:Person",
            },
            Step {
                label: "foaf:Person → greet  (subClassOf!)",
                cmd: "source urn:kernel:actions types=http://xmlns.com/foaf/0.1/Person",
                note: "no action requires foaf:Person — but it IS a schema:Person, so greet matches",
            },
        ],
    },
    Demo {
        id: "shacl",
        label: "SHACL",
        intro: "SHACL validates an RDF graph against *shapes* — which properties a node must \
                have, their types and cardinalities. `urn:shacl:validate` takes the `data` \
                (piped) and a `shapes` graph (a resource you point at); the report is itself \
                a graph, or JSON `{conforms, violations}`. Same resource, **two engines**: \
                rudof (Rust) — native in the CLI, a wasm module and the default in the \
                browser — and shacl-engine (JS), the browser's alternative, parity-tested to \
                agree. A good Account conforms; a broken one is reported; and the kernel \
                validates its *own* catalog against an `ik:Endpoint` shape (dogfooding).",
        steps: &[
            Step {
                label: "the shapes",
                cmd: "source urn:data:account-shape",
                note: "an Account needs an IRI owner and a decimal balance",
            },
            Step {
                label: "valid → conforms",
                cmd: "source urn:data:account-ok \
                      | urn:shacl:validate shapes=urn:data:account-shape as=application/json",
                note: "conforms: true, no violations",
            },
            Step {
                label: "broken → report",
                cmd: "source urn:data:account-bad \
                      | urn:shacl:validate shapes=urn:data:account-shape as=application/json",
                note: "owner is a literal (not an IRI), balance isn't a decimal → two violations",
            },
            Step {
                label: "dogfood: validate the catalog",
                cmd: "source urn:kernel:catalog \
                      | urn:shacl:validate shapes=urn:data:endpoint-shape as=application/json",
                note: "the kernel validates its OWN endpoint metadata against an ik:Endpoint shape",
            },
        ],
    },
    Demo {
        id: "lisp",
        label: "Lisp",
        intro: "Code is a resource too. `urn:lisp:eval` runs an s-expression inside the \
                kernel, where the verbs are ordinary functions — `(source \"urn:…\")` \
                re-enters resolution under *this* invocation's capability, so a program can \
                never reach past its caller's authority. The same s-expr syntax compiles to \
                RDF, and a program can be **signed**: `urn:sign:sign` produces a signature \
                graph that travels with the code, so a remote peer can verify WHO wrote it \
                before running it (wire-eval).",
        steps: &[
            Step {
                label: "evaluate",
                cmd: "source urn:lisp:eval in=\"(+ 40 2)\"",
                note: "an s-expression as a resource — the evaluator is an endpoint",
            },
            Step {
                label: "verbs are functions",
                cmd: "source urn:lisp:eval in=\"(string-upcase (source \\\"urn:data:about\\\"))\"",
                note: "(source …) re-enters the kernel: the program composes with resources, \
                       under the caller's capability",
            },
            Step {
                label: "s-expr → RDF",
                cmd: "source urn:lisp:eval in=\"(graph (quote (graph (prefix (ex \\\"http://example.org/\\\")) \
                      (ex:alice a ex:Person) (ex:alice ex:name \\\"Alice\\\"))))\"",
                note: "the same syntax compiles to canonical Turtle — code and data in one notation",
            },
            Step {
                label: "mint a signing key",
                cmd: "source urn:secret:generate into=demo-code type=ed25519",
                note: "returns the PUBLIC half; the private half stays in the keystore. \
                       NB this mints a FRESH key each time you press it",
            },
            Step {
                label: "sign a program",
                cmd: "source urn:sign:sign in=\"(+ 40 2)\" key=urn:secret:demo-code",
                note: "an RDF signature graph: algorithm, signer, value, content hash — \
                       skolemized, no blank nodes",
            },
            Step {
                label: "sign | verify",
                cmd: "source urn:sign:sign in=\"(+ 40 2)\" key=urn:secret:demo-code \
                      | urn:sign:verify in=\"(+ 40 2)\" key=urn:secret:demo-code.pub",
                note: "valid: signed by … — the signature graph pipes straight into the verifier",
            },
            Step {
                label: "verify catches a change",
                cmd: "source urn:sign:sign in=\"(+ 40 2)\" key=urn:secret:demo-code \
                      | urn:sign:verify in=\"(+ 40 3)\" key=urn:secret:demo-code.pub",
                note: "invalid: content hash mismatch — one changed character breaks it. \
                       This is what a peer checks BEFORE running shipped code: \
                       `ikigai serve quic://… --cap urn:cap:lisp:run --code-signer urn:codekey:you.pub`, \
                       then `urn:lisp:run in=<program> sig=<graph> key=<codekey>`",
            },
            Step {
                label: "named verbs, generated",
                cmd: "source urn:lisp:aliases prefix=urn:fn:",
                note: "the manifold projected as callable Lisp: `(fn-toUpper \"hi\")` instead of \
                       a URI and named arguments. GENERATED from each endpoint's own ArgSpecs — \
                       positional parameters are the REQUIRED inputs, in declaration order — so \
                       this surface cannot drift from what the kernel accepts, and a newly bound \
                       endpoint gets a verb for free",
            },
            Step {
                label: "aliases are capability-scoped",
                cmd: "source urn:lisp:aliases",
                note: "the WHOLE surface this session may invoke. It is projected from \
                       `urn:kernel:actions`, which the kernel has already narrowed to your \
                       capability — so a scoped session gets a SMALLER prelude, not a full one \
                       whose verbs fail when called. Try `cap urn:cap:kernel:inspect` first and \
                       watch it shrink",
            },
            Step {
                label: "transclude the prelude, then call it",
                cmd: "source urn:fn:compose src=urn:data:alias-demo | urn:lisp:eval",
                note: "each evaluation is ISOLATED, so definitions do not survive from one to \
                       the next — the prelude has to be IN the program. `$a{urn:lisp:aliases}` \
                       splices it in by reference, so the program stores a pointer to the \
                       manifold rather than a copy that can go stale",
            },
            Step {
                label: "the same surface, for Emacs",
                cmd: "source urn:lisp:aliases as=text/x-emacs-lisp prefix=urn:fn:",
                note: "one resource, two representations. In Emacs, `M-x ikigai-refresh-aliases` \
                       writes and loads these — `(ikigai-fn-toUpper \"hi\")` from any buffer, \
                       keybinding or org-babel block, arity-checked by Emacs itself",
            },
        ],
    },
];

/// SHACL demo resources, served as `urn:data:<id>` (Turtle) — the shapes + good/bad data the
/// SHACL demo points at. `urn:shacl:validate` sources `shapes` by reference, so these are just
/// resources like any other (rudof natively and, by default, as a wasm module in the browser;
/// shacl-engine there too on request — same results).
///
/// Every node is named, property shapes included: a blank `sh:property [ … ]` is the
/// textbook spelling, but a graph this module serves is skolemized like any other (a
/// validation report can then cite `sh:sourceShape` by a stable IRI). The Account demo lives
/// under `http://example.org/` — the reserved example namespace, because it is a demo of
/// validating *your* data, not ikigai's. The endpoint shape is the runbook's own, named
/// under `urn:runbook:shape:*`; it targets `ik:Endpoint` and `ik:title`, which the shared
/// vocabulary defines, but is not itself a vocabulary term.
static SHACL_DATA: &[(&str, &str, &str)] = &[
    (
        "account-shape",
        "Account shape (SHACL)",
        "@prefix sh: <http://www.w3.org/ns/shacl#> .\n\
         @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .\n\
         @prefix ex: <http://example.org/> .\n\
         ex:AccountShape a sh:NodeShape ;\n  \
           sh:targetClass ex:Account ;\n  \
           sh:property ex:AccountShape-owner, ex:AccountShape-balance .\n\
         ex:AccountShape-owner a sh:PropertyShape ;\n  \
           sh:path ex:owner ; sh:minCount 1 ; sh:nodeKind sh:IRI .\n\
         ex:AccountShape-balance a sh:PropertyShape ;\n  \
           sh:path ex:balance ; sh:minCount 1 ; sh:datatype xsd:decimal .\n",
    ),
    (
        "account-ok",
        "A conforming Account",
        "@prefix ex: <http://example.org/> .\n\
         ex:acct1 a ex:Account ;\n  \
           ex:owner ex:alice ;\n  \
           ex:balance \"100.50\"^^<http://www.w3.org/2001/XMLSchema#decimal> .\n",
    ),
    (
        "account-bad",
        "A violating Account",
        "@prefix ex: <http://example.org/> .\n\
         ex:acct2 a ex:Account ;\n  \
           ex:owner \"alice\" ;\n  \
           ex:balance \"lots\" .\n",
    ),
    (
        "endpoint-shape",
        "ik:Endpoint shape (SHACL)",
        "@prefix sh: <http://www.w3.org/ns/shacl#> .\n\
         @prefix ik: <https://ikigai-rs.dev/ns#> .\n\
         <urn:runbook:shape:endpoint> a sh:NodeShape ;\n  \
           sh:targetClass ik:Endpoint ;\n  \
           sh:property <urn:runbook:shape:endpoint:title> .\n\
         <urn:runbook:shape:endpoint:title> a sh:PropertyShape ;\n  \
           sh:path ik:title ; sh:minCount 1 .\n",
    ),
];

/// An RDFS alignment graph (served as `urn:data:alignment`): `foaf:Person rdfs:subClassOf
/// schema:Person`. The host parses its `rdfs:subClassOf` triples (via
/// `ikigai_rdf::subclass_axioms`) into the kernel's subclass closure, so type-aware action
/// selection reasons over it — a `foaf:Person` entity satisfies a `schema:Person` action.
/// Edit this graph (add a triple) and the available actions change.
pub const ALIGNMENT_TTL: &str = "\
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .\n\
@prefix foaf: <http://xmlns.com/foaf/0.1/> .\n\
@prefix schema: <https://schema.org/> .\n\
foaf:Person rdfs:subClassOf schema:Person .\n";

/// A JSON-LD context for the ikigai namespace, served as `urn:data:ik-context` — the
/// resource the JSON-LD demo's `compact` step compacts against. `@vocab` maps the default
/// namespace so ikigai properties compact back to bare terms; that the context shaping a
/// compaction is itself an addressable resource is the trust-boundary egress-filter framing.
const IK_CONTEXT: &str =
    r#"{"@context":{"@vocab":"https://ikigai-rs.dev/ns#","ik":"https://ikigai-rs.dev/ns#"}}"#;

/// The behavior the pages need, served as `urn:runbook:script` so no page carries inline
/// JavaScript: arrow keys along the tab strip and the status line, both by delegation on
/// the document (they survive htmx swaps). It needs neither `eval` nor `'unsafe-inline'`,
/// so it holds under `htmx.config.allowEval = false` and a strict CSP. Loading it twice is
/// harmless.
const SCRIPT: &str = r#"(function () {
  if (window.ikigaiRunbook) return;
  window.ikigaiRunbook = true;
  document.addEventListener('keydown', function (event) {
    var list = event.target.closest && event.target.closest('[data-rb-keys="tabs"]');
    if (!list) return;
    var tabs = Array.prototype.slice.call(list.querySelectorAll('[role=tab]'));
    var i = tabs.indexOf(document.activeElement);
    var k = event.key;
    if (i < 0) return;
    if (k === 'ArrowRight') i = (i + 1) % tabs.length;
    else if (k === 'ArrowLeft') i = (i + tabs.length - 1) % tabs.length;
    else if (k === 'Home') i = 0;
    else if (k === 'End') i = tabs.length - 1;
    else return;
    event.preventDefault();
    tabs[i].focus();
  });
  document.body.addEventListener('htmx:afterRequest', function (event) {
    var el = event.target.closest && event.target.closest('[data-rb-status]');
    var status = document.getElementById('rb-status');
    if (el && status) status.textContent = el.getAttribute('data-rb-status');
  });
})();
"#;

/// Where a page loads [`SCRIPT`] from, in the `/k/` form the tabs use.
const SCRIPT_SRC: &str = "/k/source urn:runbook:script";

/// The runbook space: binds `urn:runbook:<id>` for every `Demo` (private). Mount it in any
/// kernel's root (the CLI's embedded space, the in-browser kernel) and the whole
/// runbook is available, identically.
///
/// It names itself [`SPACE_ID`]: building it reads nothing and allocates no state, so every
/// call holds the same doors. The tab strip a page renders reads [`add_tab`]/[`hide_tab`]
/// state at RENDER time, not here, and the pages are served live for exactly that reason,
/// so no cache entry shared under the name can carry a stale strip.
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
                    .input(
                        ArgSpec::new("as")
                            .summary(
                                "representation: text/html (default, htmx), text/plain \
                                 (the TUI), or application/json (structured, for the TUI \
                                 to run a step by number)",
                            )
                            .class(XSD_STRING)
                            .one_of(PAGE_FACES)
                            .default_value(PAGE_FACES[0]),
                    )
                    .output(PAGE_FACES[0])
                    .output(PAGE_FACES[1])
                    .output(PAGE_FACES[2]),
            ),
        );
    }
    space = space.bind(
        Exact::new("urn:runbook:script"),
        FnEndpoint::new("runbook-script", |_inv: &Invocation<'_>| {
            Ok(repr("text/javascript", SCRIPT.to_string()).cacheable())
        })
        .with_description(
            Description::new("runbook-script")
                .title("Runbook page script")
                .summary(
                    "The script the pages load: arrow keys along the tab strip and the \
                     status line. No inline handlers, so it works without eval.",
                )
                .verb(Verb::Source)
                .verb(Verb::Meta)
                .output("text/javascript"),
        ),
    );
    // The JSON-LD demo's `compact` step compacts against this context by IRI — the context
    // is itself a resource (urn:jsonld:compact sources it). Bound here so the demo runs
    // identically in both frontends with no host-specific data.
    space = space.bind(
        Exact::new("urn:data:ik-context"),
        FnEndpoint::new("ik-context", |_inv: &Invocation<'_>| {
            Ok(repr("application/ld+json", IK_CONTEXT.to_string()).cacheable())
        })
        .with_description(
            Description::new("ik-context")
                .title("ikigai JSON-LD context")
                .summary(
                    "A JSON-LD context mapping the ikigai namespace — the resource the \
                     JSON-LD demo's compact step compacts against.",
                )
                .verb(Verb::Source)
                .verb(Verb::Meta)
                .output("application/ld+json"),
        ),
    );
    // The RDFS alignment graph the Selection demo reasons over — the host loads its
    // subClassOf triples into the kernel's closure. Served here so the demo can show it.
    space = space.bind(
        Exact::new("urn:data:alignment"),
        FnEndpoint::new("alignment", |_inv: &Invocation<'_>| {
            Ok(repr("text/turtle", ALIGNMENT_TTL.to_string()).cacheable())
        })
        .with_description(
            Description::new("alignment")
                .title("RDFS alignment graph")
                .summary(
                    "rdfs:subClassOf axioms (foaf:Person ⊑ schema:Person) the host folds into \
                     the kernel's subclass closure for type-aware action selection.",
                )
                .verb(Verb::Source)
                .verb(Verb::Meta)
                .output("text/turtle"),
        ),
    );
    // SHACL demo resources (shapes + good/bad data), each served as urn:data:<id> Turtle.
    // urn:shacl:validate sources `shapes` by reference, so these are plain resources.
    for &(id, title, ttl) in SHACL_DATA {
        space = space.bind(
            Exact::new(format!("urn:data:{id}")),
            FnEndpoint::new(format!("data-{id}"), move |_inv: &Invocation<'_>| {
                Ok(repr("text/turtle", ttl.to_string()).cacheable())
            })
            .with_description(
                Description::new(format!("data-{id}"))
                    .title(title)
                    .summary("A SHACL demo resource (shapes or data graph), served as Turtle.")
                    .verb(Verb::Source)
                    .verb(Verb::Meta)
                    .output("text/turtle"),
            ),
        );
    }
    // Toy typed "action" endpoints for the Selection demo. Each declares the RDF class its
    // input needs (`ik:class`), so `urn:kernel:actions` can match them by type — "given these
    // entities, what can I do?". The class is selection metadata; invoking just uses the
    // string value. greet needs a Person, geocode a PostalAddress, mail needs both — and each
    // input is REQUIRED in the body as well as the manifold: `inline_str(..)?` lets the
    // kernel's typed `MissingArgument` out. (They used to default to a placeholder, so the
    // demo's claim — an action is offered only when its required inputs are satisfied — was
    // enforced by the manifold and ignored by the action. Dispatch does not check ArgSpecs;
    // only `urn:kernel:validate` does.)
    space = space
        .bind(
            Exact::new("urn:action:greet"),
            FnEndpoint::new("action-greet", |inv: &Invocation<'_>| {
                Ok(repr(
                    "text/plain",
                    format!("Hello, {}!", inv.inline_str("who")?),
                ))
            })
            .with_description(action_card(
                "action-greet",
                "Greet a person",
                "Greet someone — needs a Person.",
                &[("who", "https://schema.org/Person", "the person to greet")],
            )),
        )
        .bind(
            Exact::new("urn:action:geocode"),
            FnEndpoint::new("action-geocode", |inv: &Invocation<'_>| {
                Ok(repr(
                    "text/plain",
                    format!("Geocoded: {}", inv.inline_str("address")?),
                ))
            })
            .with_description(action_card(
                "action-geocode",
                "Geocode an address",
                "Resolve a postal address to coordinates — needs a PostalAddress.",
                &[(
                    "address",
                    "https://schema.org/PostalAddress",
                    "the address to geocode",
                )],
            )),
        )
        .bind(
            Exact::new("urn:action:mail"),
            FnEndpoint::new("action-mail", |inv: &Invocation<'_>| {
                Ok(repr(
                    "text/plain",
                    format!(
                        "Mailed {} at {}",
                        inv.inline_str("to")?,
                        inv.inline_str("at")?
                    ),
                ))
            })
            .with_description(action_card(
                "action-mail",
                "Mail a person",
                "Post a letter — needs both a Person and a PostalAddress.",
                &[
                    ("to", "https://schema.org/Person", "the recipient"),
                    (
                        "at",
                        "https://schema.org/PostalAddress",
                        "the delivery address",
                    ),
                ],
            )),
        );
    // Named LAST: since ikigai-core 0.1.89 a later `bind` drops the name.
    space.named(space_iri("runbook"))
}

/// Build an action endpoint's self-description: a Source/Meta endpoint whose every input is
/// a *required, typed* argument (`ik:class`), so `urn:kernel:actions` matches it by type.
fn action_card(id: &str, title: &str, summary: &str, inputs: &[(&str, &str, &str)]) -> Description {
    let mut d = Description::new(id.to_string())
        .title(title.to_string())
        .summary(summary.to_string())
        .verb(Verb::Source)
        .verb(Verb::Meta)
        .output("text/plain");
    for (name, class, summary) in inputs {
        d = d.input(ArgSpec::new(*name).class(*class).summary(*summary));
    }
    d
}

/// The [`PAGE_FACES`] entry an `as` value names, or `None`. A face is matched on the
/// media type's essence: a parameter (`; charset=utf-8`) and ASCII case do not make a
/// different face (RFC 9110 §8.3.1), so `Text/Plain; charset=utf-8` is `text/plain`.
fn page_face(as_type: &str) -> Option<&'static str> {
    let essence = as_type.split(';').next().unwrap_or_default().trim();
    PAGE_FACES
        .into_iter()
        .find(|face| face.eq_ignore_ascii_case(essence))
}

/// Render `demo` per the requested `as` type: htmx HTML (the default, when `as` is
/// absent), `text/plain` for the terminal, or the structured `application/json`. The
/// branches are exactly [`PAGE_FACES`], and an `as` outside that list is REFUSED with
/// `InvalidArgument` naming the three, as the manifold's `one_of` says: answering it with
/// the default face would read as success to a caller that asked for something else
/// (ledger #210). The charset the served type carries is a parameter of the bytes, not a
/// fourth face.
fn render(demo: &Demo, inv: &Invocation<'_>) -> Result<Representation> {
    let as_type = match inv.inline_str("as") {
        Ok(value) => value,
        Err(Error::MissingArgument(_)) => PAGE_FACES[0],
        Err(other) => return Err(other),
    };
    match page_face(as_type) {
        Some("application/json") => {
            // Structured form: `{ id, label, intro, steps: [{ label, cmd, note }] }` — the
            // TUI sources this to render the page and run a step by its number.
            let json = serde_json::to_string(demo)
                .map_err(|e| Error::Endpoint(format!("runbook json: {e}")))?;
            Ok(repr("application/json", json))
        }
        Some("text/plain") => Ok(repr("text/plain", render_text(demo))),
        Some("text/html") => Ok(repr("text/html", render_html(demo))),
        _ => Err(Error::InvalidArgument {
            name: "as".to_string(),
            detail: format!(
                "`{as_type}` is not a runbook page face; `as` is one of {}",
                PAGE_FACES.join(", ")
            ),
        }),
    }
}

/// Prose for the HTML face: escaped, with each backtick pair turned into `<code>`. An
/// unpaired backtick stays a literal character.
fn prose(s: &str) -> String {
    let parts: Vec<&str> = s.split('`').collect();
    let mut out = String::new();
    for (i, part) in parts.iter().enumerate() {
        if i % 2 == 1 && i + 1 < parts.len() {
            out.push_str(&format!("<code>{}</code>", html_text(part)));
        } else {
            // Text between pairs; an unpaired final tick is kept as a character.
            if i > 0 && i % 2 == 1 {
                out.push('`');
            }
            out.push_str(&html_text(part));
        }
    }
    out
}

fn repr(media: &str, body: String) -> Representation {
    Representation::new(
        ReprType::new(media).with_param("charset", "utf-8"),
        body.into_bytes(),
    )
}

/// Host-registered extra tabs `(id, label)`, appended to the strip after the built-in
/// demos. Lets a host add a tab the shared module doesn't know about — e.g. the web
/// demo's browser-only "Identity" tab, bound as `urn:runbook:identity`. Process-global,
/// the same convention as the host toggles; the native CLI simply never registers any.
fn extra_tabs() -> &'static std::sync::Mutex<Vec<(String, String)>> {
    static TABS: std::sync::OnceLock<std::sync::Mutex<Vec<(String, String)>>> =
        std::sync::OnceLock::new();
    TABS.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

/// Host-hidden tab ids, filtered out of the strip wherever it is rendered. The
/// subtractive twin of [`extra_tabs`], and process-global for the same reason.
fn hidden_tabs() -> &'static std::sync::Mutex<Vec<String>> {
    static HIDDEN: std::sync::OnceLock<std::sync::Mutex<Vec<String>>> = std::sync::OnceLock::new();
    HIDDEN.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

/// Register an extra tab so it appears in the runbook strip on every panel. The host
/// also binds `urn:runbook:<id>`; that endpoint's `text/html` representation should lead
/// with [`render_tab_strip`]`(<id>)` so the strip stays identical across tabs.
pub fn add_tab(id: impl Into<String>, label: impl Into<String>) {
    let id = id.into();
    let mut tabs = extra_tabs().lock().expect("runbook tabs");
    // Idempotent: a host may build its kernel more than once (the in-page singleton,
    // the server, tests) — register the tab at most once.
    if !tabs.iter().any(|(existing, _)| existing == &id) {
        tabs.push((id, label.into()));
    }
}

/// Hide a tab — the subtractive mirror of [`add_tab`]. A host that cannot serve a
/// built-in page says so once, and the tab stops appearing in every strip this crate
/// renders (HTML and text alike).
///
/// The case that motivated it: the in-browser WASM kernel does not link `ikigai-lisp`
/// (Steel doesn't go to wasm), so its **Lisp** tab rendered a panel whose every step
/// answered `no endpoint resolved for urn:lisp:eval`. `hide_tab("lisp")` removes the
/// offer.
///
/// Semantics, all three deliberate:
///
/// * **An unknown id is accepted silently.** Hiding is recorded as an id, not resolved
///   against a tab, so `hide_tab` and [`add_tab`] commute — hiding a host tab before
///   registering it gives the same strip as hiding it after. The cost is real and is not
///   papered over: a **typo hides nothing and says nothing**, and the tab quietly comes
///   back. [`tab_ids`] is the seam for catching that — a host can assert on the strip it
///   actually gets.
/// * **Hidden wins over active.** If the hidden page is the one being rendered (a host
///   can still be handed `urn:runbook:lisp` directly), the strip omits it and no tab is
///   marked selected. The alternative — showing it only while active — would put a tab
///   for an unservable page back in front of the very user who reached it.
/// * **Hiding is presentation, not unbinding.** [`space`] still binds every built-in
///   page, so a hidden resource stays resolvable by name. Making the bound space depend
///   on this global would make a kernel's contents depend on whether `hide_tab` ran
///   before or after `space()` — the order-dependence this API exists to avoid.
///
/// **The weakness, stated plainly:** `hide_tab` requires the host to *know* what it
/// cannot serve. It fixes the Lisp tab; it does not close the class. The honest version
/// is a strip that probes the kernel and omits whatever does not resolve — considered
/// and deferred, so the next tab a host can't bind regresses in exactly this way. Treat
/// this as the manual valve it is.
///
/// ```
/// ikigai_runbook::add_tab("identity", "Identity");
/// ikigai_runbook::hide_tab("lisp");
///
/// let strip = ikigai_runbook::render_tab_strip("basics");
/// assert!(strip.contains("urn:runbook:identity"));
/// assert!(!strip.contains("urn:runbook:lisp"));
/// ```
pub fn hide_tab(id: impl Into<String>) {
    let id = id.into();
    let mut hidden = hidden_tabs().lock().expect("runbook hidden tabs");
    // Idempotent for the same reason `add_tab` is: hosts build kernels more than once.
    if !hidden.iter().any(|existing| existing == &id) {
        hidden.push(id);
    }
}

/// The ids currently in the strip, in tab order: the built-ins plus any [`add_tab`]s,
/// minus any [`hide_tab`]s. The observable form of what the strips render — a host that
/// wants a mistyped [`hide_tab`] to be loud can assert against this.
pub fn tab_ids() -> Vec<String> {
    let hidden = snapshot_hidden();
    let extra = snapshot_extra();
    DEMOS
        .iter()
        .map(|d| d.id.to_string())
        .chain(extra.into_iter().map(|(id, _)| id))
        .filter(|id| !hidden.contains(id))
        .collect()
}

/// Copy the extra tabs out from under the lock. Both snapshot helpers exist so that the
/// rendering runs with *no* lock held: `render_tab_strip` is called from inside a
/// resolution, and doing fallible or re-entrant work under a process-global lock is how a
/// host gets wedged — that mistake cost the web demo its Control panel this week
/// (`ikigai-time` #303). Cloning two short `Vec<String>`s per strip is the cheap side of
/// that trade.
fn snapshot_extra() -> Vec<(String, String)> {
    extra_tabs().lock().expect("runbook tabs").clone()
}

fn snapshot_hidden() -> Vec<String> {
    hidden_tabs().lock().expect("runbook hidden tabs").clone()
}

/// The htmx tab strip — the built-in demos plus any host [`add_tab`]s, minus any
/// [`hide_tab`]s — with `active` marked `selected`. Public so a host's extra-tab endpoint
/// renders the identical strip (HATEOAS: every tab carries the whole strip, so "which tab
/// is active" lives in the returned HTML, not client state).
///
/// If `active` is a hidden id the strip simply has no selected tab; the panel below it
/// still renders, because hiding a tab does not unbind its resource.
pub fn render_tab_strip(active: &str) -> String {
    // Snapshot first, then render: no allocation-heavy formatting and no re-entrancy
    // while the process-global locks are held.
    let hidden = snapshot_hidden();
    let extra = snapshot_extra();

    let all: Vec<(String, String)> = DEMOS
        .iter()
        .map(|d| (d.id.to_string(), d.label.to_string()))
        .chain(extra)
        .filter(|(id, _)| !hidden.contains(id))
        .collect();
    // Roving tabindex: the selected tab is the one Tab stop of the strip; with none
    // selected (the active page is hidden) the first tab stands in, so the strip stays
    // reachable.
    let stop = all.iter().position(|(id, _)| id == active).unwrap_or(0);

    // The landmark is the `<nav>`; the tabs sit in a `tablist` inside it. Arrow keys, Home
    // and End move focus along the tabs (activation stays on Enter and Space), and each
    // tab has a stable id so htmx puts focus back on it after the swap replaces the strip.
    let mut tabs = String::from(
        "<nav class=\"rb-tabs\" aria-label=\"Runbook pages\">\
         <div role=\"tablist\" aria-label=\"Runbook pages\" \
         data-rb-keys=\"tabs\">",
    );
    for (n, (id, label)) in all.iter().enumerate() {
        let selected = id == active;
        tabs.push_str(&format!(
            "<button role=\"tab\" id=\"rb-tab-{id}\" class=\"rb-tab{cls}\" \
             aria-selected=\"{sel}\" aria-controls=\"rb-panel\" tabindex=\"{ti}\" \
             hx-get=\"/k/source urn:runbook:{id} as=text/html\" \
             hx-target=\"#runbook\" hx-swap=\"innerHTML\">{label}</button>",
            cls = if selected { " selected" } else { "" },
            sel = selected,
            ti = if n == stop { 0 } else { -1 },
            id = attr(id),
            label = html_text(label),
        ));
    }
    tabs.push_str("</div></nav>");
    tabs
}

/// The htmx HATEOAS fragment: the tab strip (active tab marked) followed by the
/// active demo's panel. Switching tabs `hx-get`s another `urn:runbook:*` into the
/// `#runbook` container; running a step `hx-get`s its command into `#rb-out`. The
/// host adapter maps `/k/<command>` → `engine.eval`. No client-side state.
fn render_html(active: &Demo) -> String {
    let tabs = render_tab_strip(active.id);

    let mut steps = String::from("<ol class=\"rb-steps\">");
    for step in active.steps {
        let cmd_attr = attr(step.cmd);
        let cmd_text = html_text(step.cmd);
        let label_attr = attr(step.label);
        steps.push_str(&format!(
            "<li><button class=\"rb-step\" aria-label=\"{label_attr}: {cmd_attr}\" \
             hx-get=\"/k/{cmd_attr}\" hx-target=\"#rb-out\" hx-swap=\"beforeend\" \
             data-rb-status=\"Ran {label_attr}\">\
             {label}</button> <code class=\"rb-cmd\">{cmd_text}</code>\
             <span class=\"rb-note\">{note}</span></li>",
            label = html_text(step.label),
            note = prose(step.note),
        ));
    }
    steps.push_str("</ol>");

    format!(
        "{tabs}<section id=\"rb-panel\" class=\"rb-panel\" role=\"tabpanel\" \
         aria-labelledby=\"rb-heading\" tabindex=\"0\">\
         <h2 id=\"rb-heading\" class=\"rb-heading\">{title}</h2>\
         <p class=\"rb-intro\">{intro}</p>{steps}\
         <div class=\"rb-outbar\">\
           <button class=\"rb-clear\" hx-get=\"/k/clear\" hx-target=\"#rb-out\" \
             hx-swap=\"innerHTML\" \
             data-rb-status=\"Output cleared\">\
             clear output</button>\
         </div>\
         <p id=\"rb-status\" class=\"rb-status\" role=\"status\"></p>\
         <pre id=\"rb-out\" class=\"rb-out\" role=\"region\" aria-label=\"Output\" tabindex=\"0\"></pre>\
         </section>\
         <script src=\"{src}\"></script>",
        src = SCRIPT_SRC,
        title = html_text(active.label),
        intro = prose(active.intro),
    )
}

/// The terminal rendering: the tab list, the intro, and the steps as a numbered,
/// runnable list. (The TUI runs a step by issuing its command; it can't run htmx.)
fn render_text(active: &Demo) -> String {
    let mut out = String::new();
    // The text face honours `hide_tab` too: a host that cannot serve a page shouldn't
    // offer it in the TUI either.
    let hidden = snapshot_hidden();
    let tabs: Vec<String> = DEMOS
        .iter()
        .filter(|d| !hidden.iter().any(|h| h == d.id))
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

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use ikigai_core::{ArgRef, Capability, Iri, Kernel, Request};
    use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

    /// `add_tab`/`hide_tab` write process-global state, so the tests that touch them run
    /// one at a time. Nothing un-hides — a hidden id stays hidden for the rest of the
    /// binary — so no test may assume the presence of a tab another test hides.
    fn serial() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Source a runbook page through a real kernel, as the hosts do.
    fn source(iri: &str, as_type: &str) -> String {
        let request = Request::new(Verb::Source, Iri::parse(iri).unwrap())
            .with_arg("as", ArgRef::Inline(as_type.as_bytes().to_vec()));
        let kernel = Kernel::new(Arc::new(space()));
        let rep = block_on(kernel.issue(request, &Capability::root())).unwrap();
        String::from_utf8(rep.bytes).unwrap()
    }

    /// Ledger #1643: the structure of the HTML face (tabs pattern, landmark, heading,
    /// button names, output region, inline code).
    #[test]
    fn the_html_face_follows_the_tabs_pattern() {
        let _guard = serial();
        let html = source("urn:runbook:basics", "text/html");
        // A landmark holding a tablist; arrow keys handled on it.
        assert!(html.contains("<nav class=\"rb-tabs\" aria-label="));
        assert!(html.contains("<div role=\"tablist\""));
        assert!(!html.contains("<nav class=\"rb-tabs\" role="));
        assert!(html.contains("data-rb-keys=\"tabs\""));
        // Roving tabindex: exactly one tab is a Tab stop, the selected one.
        assert_eq!(
            html.matches("role=\"tab\"").count(),
            html.matches("aria-controls=\"rb-panel\"").count()
        );
        assert_eq!(html.matches("tabindex=\"0\" hx-get=\"/k/source").count(), 1);
        assert!(html.contains("id=\"rb-tab-basics\" class=\"rb-tab selected\" aria-selected=\"true\" aria-controls=\"rb-panel\" tabindex=\"0\""));
        // The panel is named by a heading, which exists.
        assert!(html.contains("id=\"rb-panel\""));
        assert!(html.contains("aria-labelledby=\"rb-heading\""));
        assert!(html.contains("<h2 id=\"rb-heading\" class=\"rb-heading\">Basics</h2>"));
        // Step names carry the command.
        for step in DEMOS[0].steps {
            assert!(html.contains(&format!(
                "aria-label=\"{}: {}\"",
                attr(step.label),
                attr(step.cmd)
            )));
        }
        // No live region that announces whole answers; a status line does the announcing.
        assert!(!html.contains("aria-live"));
        assert!(html.contains("<p id=\"rb-status\" class=\"rb-status\" role=\"status\">"));
        // Backticks become code, not characters.
        assert!(!html.contains('`'));
        assert!(html.contains("<code>|</code>"));
    }

    /// Ledger #1812: the faces carry no inline handlers (htmx evaluates `hx-on` with eval,
    /// which `allowEval: false` and a strict CSP forbid); the behavior is a served script.
    #[test]
    fn the_html_face_has_no_inline_handlers() {
        let _guard = serial();
        for demo in DEMOS {
            let html = source(&format!("urn:runbook:{}", demo.id), "text/html");
            assert!(!html.contains("hx-on"), "{}: {html}", demo.id);
            let tags: String = html
                .split('>')
                .filter_map(|t| t.split('<').nth(1))
                .collect();
            assert!(
                !tags
                    .split_whitespace()
                    .any(|w| w.starts_with("on") && w.contains('=')),
                "{}: inline on* attribute",
                demo.id
            );
            assert!(!html.contains("javascript:"));
            assert_eq!(html.matches("<script").count(), 1);
            assert!(html.contains(&format!("<script src=\"{SCRIPT_SRC}\"></script>")));
            assert!(html.contains("data-rb-status=\"Output cleared\""));
        }
        let strip = render_tab_strip("basics");
        assert!(!strip.contains("hx-on") && !strip.contains("<script"));
        let js = source("urn:runbook:script", "text/javascript");
        assert!(js.contains("ArrowRight") && js.contains("htmx:afterRequest"));
        assert!(!js.contains("eval(") && !js.contains("Function("));
    }

    /// With the active page hidden, the first tab is still a Tab stop.
    #[test]
    fn a_strip_with_nothing_selected_keeps_one_tab_stop() {
        let _guard = serial();
        let strip = render_tab_strip("no-such-page");
        assert_eq!(strip.matches("tabindex=\"0\"").count(), 1);
        assert!(strip.contains("id=\"rb-tab-basics\" class=\"rb-tab\" aria-selected=\"false\" aria-controls=\"rb-panel\" tabindex=\"0\""));
    }

    #[test]
    fn prose_pairs_backticks_and_escapes() {
        assert_eq!(prose("a `<b>` c"), "a <code>&lt;b&gt;</code> c");
        assert_eq!(prose("odd ` tick"), "odd ` tick");
        assert_eq!(prose("`x` and `y"), "<code>x</code> and `y");
    }

    /// Ledger #1644: a tab label (or id) from untrusted input renders as text, not markup.
    #[test]
    fn a_tab_label_holding_markup_renders_as_text() {
        add_tab("evil1644", "<script>alert(1)</script>\"&");
        let html = source("urn:runbook:basics", "text/html");
        assert!(!html.contains("<script>"), "label injected markup: {html}");
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;&quot;&amp;"));
    }

    /// A command holding `'` and `"` is escaped in the attributes and in the text.
    #[test]
    fn a_command_with_quotes_is_escaped_in_attribute_and_text() {
        static STEPS: [Step; 1] = [Step {
            label: "Q'\"",
            cmd: "echo 'a' \"b\"",
            note: "n",
        }];
        let demo = Demo {
            id: "q",
            label: "Q",
            intro: "i",
            steps: &STEPS,
        };
        let html = render_html(&demo);
        assert!(
            html.contains("hx-get=\"/k/echo &#39;a&#39; &quot;b&quot;\""),
            "{html}"
        );
        assert!(
            html.contains("aria-label=\"Q&#39;&quot;: echo &#39;a&#39; &quot;b&quot;\""),
            "{html}"
        );
        assert!(
            html.contains("<code class=\"rb-cmd\">echo &#39;a&#39; "),
            "{html}"
        );
        assert!(!html.contains("echo 'a'"), "{html}");
    }

    fn button(id: &str) -> String {
        format!("urn:runbook:{id} as=text/html")
    }

    /// Issue a Source for a page with `as` set to `arg`, returning the raw result.
    fn try_source(iri: &str, arg: ArgRef) -> Result<Representation> {
        let request = Request::new(Verb::Source, Iri::parse(iri).unwrap()).with_arg("as", arg);
        let kernel = Kernel::new(Arc::new(space()));
        block_on(kernel.issue(request, &Capability::root()))
    }

    /// Ledger #210: the manifold says `as` is `one_of` [`PAGE_FACES`], so the body must
    /// agree. An `as` outside the list used to be answered with the HTML face, which a
    /// caller asking for Turtle (or mistyping `text/plian`) would read as success. Every
    /// page refuses it with a typed `InvalidArgument` naming `as` and the three faces.
    #[test]
    fn an_as_outside_the_page_faces_is_refused_by_name() {
        for demo in DEMOS {
            let iri = format!("urn:runbook:{}", demo.id);
            for unknown in [
                "text/turtle",
                "text/plian",
                "text/htmlx",
                "application/jsonx",
                "",
            ] {
                match try_source(&iri, ArgRef::Inline(unknown.as_bytes().to_vec())) {
                    Err(Error::InvalidArgument { name, detail }) => {
                        assert_eq!(name, "as", "{iri} as={unknown:?}");
                        for face in PAGE_FACES {
                            assert!(
                                detail.contains(face),
                                "{iri} as={unknown:?}: detail {detail:?} omits {face}"
                            );
                        }
                    }
                    other => panic!("{iri} as={unknown:?} was not refused: {other:?}"),
                }
            }
        }
    }

    /// A face is matched on its essence: a media-type parameter (`charset`) and ASCII
    /// case are not a different face (RFC 9110 §8.3.1), so they still serve.
    #[test]
    fn a_face_with_a_parameter_or_other_case_still_serves() {
        for (asked, served) in [
            ("text/plain; charset=utf-8", "text/plain"),
            ("Text/HTML", "text/html"),
            (" application/json ", "application/json"),
        ] {
            let rep = try_source(
                "urn:runbook:basics",
                ArgRef::Inline(asked.as_bytes().to_vec()),
            )
            .unwrap_or_else(|e| panic!("as={asked:?} refused: {e}"));
            assert_eq!(rep.repr_type.media_type, served, "as={asked:?}");
        }
    }

    /// An `as` that is present but unusable (by reference rather than inline) is refused
    /// too, rather than read as absent and answered with the default face.
    #[test]
    fn a_by_reference_as_is_refused_not_defaulted() {
        let by_ref = ArgRef::Reference(Iri::parse("urn:data:ik-context").unwrap());
        match try_source("urn:runbook:basics", by_ref) {
            Err(Error::InvalidArgument { name, .. }) => assert_eq!(name, "as"),
            other => panic!("a by-reference `as` was not refused: {other:?}"),
        }
    }

    /// The motivating case: a host that cannot serve `urn:lisp:eval` hides the Lisp tab,
    /// and it stops being offered — in the HTML strip and the text face alike — while its
    /// neighbours keep theirs. This is the only test that hides `lisp`.
    #[test]
    fn hide_tab_removes_the_tab_everywhere_it_is_rendered() {
        let _guard = serial();

        assert!(render_tab_strip("basics").contains(&button("lisp")));
        assert!(source("urn:runbook:basics", "text/plain").contains("Lisp"));

        hide_tab("lisp");

        let strip = render_tab_strip("basics");
        assert!(!strip.contains(&button("lisp")), "hidden tab still offered");
        assert!(strip.contains(&button("basics")), "neighbour tab lost");
        assert!(!tab_ids().iter().any(|id| id == "lisp"));
        assert!(tab_ids().iter().any(|id| id == "basics"));

        // The text face honours it too.
        let text = source("urn:runbook:basics", "text/plain");
        assert!(!text.lines().next().unwrap().contains("Lisp"));
        assert!(text.lines().next().unwrap().contains("Basics"));
    }

    /// Hiding is presentation, not unbinding: the page still resolves (a host can be
    /// handed `urn:runbook:<id>` directly), and when the hidden page IS the active one
    /// the strip omits it rather than showing an unservable tab — so nothing is marked
    /// selected. Uses `http` so it stays independent of the `lisp` test's ordering.
    #[test]
    fn a_hidden_page_still_resolves_and_selects_nothing() {
        let _guard = serial();
        hide_tab("http");

        let html = source("urn:runbook:http", "text/html");
        assert!(html.contains("rb-panel"), "hidden page stopped resolving");
        assert!(
            !html.contains(&button("http")),
            "hidden tab re-offered as active"
        );
        assert!(
            !html.contains("aria-selected=\"true\""),
            "hidden tab selected"
        );
        // The rest of the strip is intact, so there is a way back out of the page.
        assert!(html.contains(&button("basics")));
    }

    /// `hide_tab` and `add_tab` commute: hiding before registering gives the same strip
    /// as hiding after. That is what makes the id-not-tab bookkeeping worth its cost.
    #[test]
    fn hide_and_add_commute_in_either_order() {
        let _guard = serial();
        let before = render_tab_strip("basics");

        add_tab("alpha", "Alpha");
        hide_tab("alpha");
        let add_then_hide = render_tab_strip("basics");

        hide_tab("beta");
        add_tab("beta", "Beta");
        let hide_then_add = render_tab_strip("basics");

        assert_eq!(before, add_then_hide, "add-then-hide changed the strip");
        assert_eq!(before, hide_then_add, "hide-then-add changed the strip");
        assert!(!render_tab_strip("basics").contains("Alpha"));
        assert!(!render_tab_strip("basics").contains("Beta"));
    }

    /// An unknown id is accepted silently — the price of commuting with `add_tab`. The
    /// documented consequence is that a TYPO is a no-op, and this is what that looks
    /// like: `tab_ids` is the seam a host can assert on to catch it.
    #[test]
    fn hiding_an_unknown_id_changes_nothing() {
        let _guard = serial();
        let before = render_tab_strip("basics");
        let ids_before = tab_ids();

        hide_tab("lissp"); // the typo a host would actually make

        assert_eq!(before, render_tab_strip("basics"));
        assert_eq!(ids_before, tab_ids());
    }

    /// Hiding works on a host's own `add_tab` too, not just the built-ins.
    #[test]
    fn hide_tab_also_hides_a_host_registered_tab() {
        let _guard = serial();
        add_tab("gamma", "Gamma");
        assert!(render_tab_strip("basics").contains(&button("gamma")));

        hide_tab("gamma");
        assert!(!render_tab_strip("basics").contains(&button("gamma")));
    }

    /// Idempotence, matching `add_tab`: a host that builds its kernel twice hides once.
    #[test]
    fn hide_tab_is_idempotent() {
        let _guard = serial();
        add_tab("delta", "Delta");
        hide_tab("delta");
        let once = render_tab_strip("basics");
        hide_tab("delta");
        assert_eq!(once, render_tab_strip("basics"));
        assert_eq!(
            1,
            hidden_tabs()
                .lock()
                .unwrap()
                .iter()
                .filter(|h| *h == "delta")
                .count()
        );
    }
}
