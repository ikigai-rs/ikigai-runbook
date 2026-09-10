//! The module recipe as one test: `ikigai-conformance` walks the twenty-one
//! endpoints [`ikigai_runbook::space`] binds — twelve `runbook-<id>` pages, six
//! constant graphs (`urn:data:*`), three toy actions (`urn:action:*`) — and
//! reports every violation at once.
//!
//! ## The kernel is the module's own space, with nothing added
//!
//! This crate reads no file, no clock and no network: every page is an in-crate
//! table rendered on demand, every graph is a string constant, every toy action
//! is a function of its inputs. So the kernel under test is `space()` alone —
//! no fixture endpoint, no fixture file, no clock — and the walk fires nothing
//! with a side effect (there is no Sink or Delete here: a runbook step's command
//! is text in an `hx-get`, and the HOST's `/k/<command>` adapter runs it, under
//! the session's capability; this crate only renders the button).
//!
//! ## Declarations, and why
//!
//! - **`pure` + `cacheable` on the six constant graphs.** Each is `.cacheable()`
//!   with an empty thread set — correct, because a constant has nothing to cut —
//!   and the suite rightly refuses to take that on faith, so it is declared.
//! - **Nothing on the twelve pages.** They are served live (`Expiry::Always`)
//!   ON PURPOSE: the tab strip is process-global host state (`add_tab` /
//!   `hide_tab`) that no golden thread tracks and no cut can reach, so a cached
//!   page would serve a strip the host has since withdrawn.
//!   [`pages_are_live_because_the_strip_is_host_state`] pins the decision the
//!   suite cannot see (its PENDING #22: a clean report cannot tell "live by
//!   decision" from "never thought about it").
//! - **Nothing on the three toy actions.** Pure functions of their inputs, left
//!   live: they stand in for real actions in a demo about SELECTION, and one of
//!   them is named `mail`.
//! - **`namespace("http://example.org/")`.** The SHACL demo's Account data is a
//!   demo of validating *your* data, so it lives under the reserved example
//!   namespace by design; the README says so.
//!
//! No opt-outs; NAMES runs (every id is kebab-case).
//!
//! ## What the suite cannot hold and this file pins by hand
//!
//! - **Declared outputs are the media types served, both directions, driven
//!   from `as`'s `one_of`** ([`declared_outputs_are_the_media_types_served`]):
//!   0.1.0 compares only RDF faces, and only once declared (PENDING
//!   #11/#31/#79). The `text/plain` and `application/json` faces were served
//!   under `as=` for thirteen releases with `text/html` as the only output.
//! - **A required input is required** ([`a_toy_action_refuses_a_missing_input`]):
//!   the kernel checks ArgSpecs only in `urn:kernel:validate`, never at
//!   dispatch, so an endpoint that quietly ran without a required input made
//!   the manifold lie the other way (PENDING #49). All three did, with
//!   placeholder defaults, until this arc.
//! - **A shape's `sh:path` / `sh:targetClass` objects are terms too**
//!   ([`the_shapes_are_skolemized_and_reference_only_defined_terms`]):
//!   VOCABULARY sees predicates and `rdf:type` objects, so the `ik:` terms the
//!   endpoint shape TARGETS were never checked — nor was the shape's own subject
//!   IRI, which sat under `ik:` undefined.
//! - **The JSON-LD context has zero triples** — a context document, not a
//!   graph — so SKOLEM-RDF and VOCABULARY are vacuous on it by nature (PENDING
//!   #26). [`the_constant_graphs_are_cached_with_no_thread_by_design`] pins the
//!   contract `urn:jsonld:compact` actually reads: a JSON object with an
//!   `@context` whose `@vocab` is the ikigai namespace.

use std::collections::BTreeSet;
use std::sync::Arc;

use ikigai_conformance::{rdf, Report, Suite};
use ikigai_core::{ArgRef, Capability, Error, Expiry, Iri, Kernel, Representation, Request, Verb};
use ikigai_runbook::PAGE_FACES;

/// The twelve built-in pages, by tab id (`urn:runbook:<id>`, description
/// `runbook-<id>`).
const PAGES: [&str; 12] = [
    "basics",
    "piping",
    "http",
    "constraints",
    "zerotrust",
    "linkeddata",
    "transrept",
    "sniff",
    "jsonld",
    "selection",
    "shacl",
    "lisp",
];

/// The six constant graphs: (description id, IRI, media type served).
const GRAPHS: [(&str, &str, &str); 6] = [
    ("ik-context", "urn:data:ik-context", "application/ld+json"),
    ("alignment", "urn:data:alignment", "text/turtle"),
    (
        "data-account-shape",
        "urn:data:account-shape",
        "text/turtle",
    ),
    ("data-account-ok", "urn:data:account-ok", "text/turtle"),
    ("data-account-bad", "urn:data:account-bad", "text/turtle"),
    (
        "data-endpoint-shape",
        "urn:data:endpoint-shape",
        "text/turtle",
    ),
];

/// The three toy actions: (IRI, the required inputs with a value each).
const ACTIONS: [(&str, &[(&str, &str)]); 3] = [
    ("urn:action:greet", &[("who", "Ada")]),
    ("urn:action:geocode", &[("address", "1 Main St")]),
    ("urn:action:mail", &[("to", "Ada"), ("at", "1 Main St")]),
];

/// The reserved example namespace the Account demo lives under (registered).
const EXAMPLE_NS: &str = "http://example.org/";
/// The runbook's own scheme for the endpoint shape's nodes.
const SHAPE_NS: &str = "urn:runbook:shape:";
const IK: &str = "https://ikigai-rs.dev/ns#";
const SH_PATH: &str = "http://www.w3.org/ns/shacl#path";
const SH_TARGET_CLASS: &str = "http://www.w3.org/ns/shacl#targetClass";

fn kernel() -> Kernel {
    Kernel::new(Arc::new(ikigai_runbook::space()))
}

fn request(verb: Verb, iri: &str, args: &[(&str, &str)]) -> Request {
    let mut request = Request::new(verb, Iri::parse(iri).expect("a valid IRI"));
    for (name, value) in args {
        request = request.with_arg(*name, ArgRef::Inline(value.as_bytes().to_vec()));
    }
    request
}

fn source(iri: &str) -> Request {
    request(Verb::Source, iri, &[])
}

fn issue(kernel: &Kernel, request: Request) -> ikigai_core::Result<Representation> {
    futures::executor::block_on(kernel.issue(request, &Capability::root()))
}

fn resolve(kernel: &Kernel, request: Request) -> Representation {
    issue(kernel, request).unwrap_or_else(|e| panic!("resolution failed: {e}"))
}

fn text(repr: &Representation) -> String {
    String::from_utf8(repr.bytes.clone()).expect("UTF-8")
}

fn page_iri(id: &str) -> String {
    format!("urn:runbook:{id}")
}

/// The suite, configured for this module (the file docs say why each line).
fn suite() -> Suite {
    let mut suite = Suite::new().namespace(EXAMPLE_NS);
    for (id, _, _) in GRAPHS {
        suite = suite.pure(id).cacheable(id);
    }
    suite
}

/// The walk saw every endpoint `space()` binds — one Source action each; Meta is
/// not an action — and skipped nothing. A twenty-second endpoint bound without a
/// line here would be held to a weaker standard.
fn assert_shape(report: &Report) {
    assert_eq!(
        report.endpoints, 21,
        "12 pages + 6 graphs + 3 toy actions: {report}"
    );
    assert_eq!(report.actions, 21, "one Source each: {report}");
    assert_eq!(
        report.checks.skipped().count(),
        0,
        "every check runs: {report}"
    );
}

#[test]
fn conforms() {
    let kernel = kernel();
    let report = suite().run_blocking(&kernel);
    // Printed even when clean (`--nocapture`): the report is the record.
    eprintln!("{report}");
    assert!(report.is_clean(), "{report}");
    assert_shape(&report);
}

/// What `ikigai-conformance` 0.1.0 does not check (PENDING #11/#31/#79): a
/// declared output that is not an RDF face is never compared with what the
/// action serves, and a face served only under `as=` is never probed unless
/// declared. Both directions, by hand: on every page `as`'s `one_of` IS the
/// declared output list, `as` omitted serves the first of them, and each value
/// serves its own type; on every other endpoint `as` omitted serves a declared
/// output.
#[test]
fn declared_outputs_are_the_media_types_served() {
    let kernel = kernel();
    let faces: BTreeSet<String> = PAGE_FACES.iter().map(|f| f.to_string()).collect();
    for id in PAGES {
        let iri = page_iri(id);
        let description = kernel
            .describe(&Iri::parse(&iri).expect("a valid IRI"))
            .unwrap_or_else(|| panic!("{iri} describes itself"));
        let spec = description
            .action_specs()
            .into_iter()
            .find(|a| a.verb == Verb::Source)
            .expect("Source is declared");
        let declared: BTreeSet<String> = spec
            .outputs
            .iter()
            .map(|o| rdf::bare_media_type(o))
            .collect();
        let as_spec = spec
            .inputs
            .iter()
            .find(|i| i.name == "as")
            .expect("`as` is declared");
        let one_of: BTreeSet<String> = as_spec.one_of.iter().cloned().collect();
        assert_eq!(one_of, faces, "{iri}: `as` lists exactly the faces");
        assert_eq!(declared, faces, "{iri}: the faces are the outputs");
        assert!(
            !as_spec.required,
            "{iri}: `as` has a default, so it is optional"
        );
        assert_eq!(as_spec.default.as_deref(), Some(PAGE_FACES[0]));

        let served = resolve(&kernel, source(&iri));
        assert_eq!(
            rdf::bare_media_type(&served.repr_type.media_type),
            PAGE_FACES[0],
            "{iri}: the default face"
        );
        for face in PAGE_FACES {
            let served = resolve(&kernel, request(Verb::Source, &iri, &[("as", face)]));
            assert_eq!(
                rdf::bare_media_type(&served.repr_type.media_type),
                face,
                "{iri} as={face}"
            );
        }
    }

    let mut others: Vec<(String, Vec<(&str, &str)>)> = GRAPHS
        .iter()
        .map(|(_, iri, _)| (iri.to_string(), Vec::new()))
        .collect();
    others.extend(
        ACTIONS
            .iter()
            .map(|(iri, args)| (iri.to_string(), args.to_vec())),
    );
    for (iri, args) in others {
        let description = kernel
            .describe(&Iri::parse(&iri).expect("a valid IRI"))
            .unwrap_or_else(|| panic!("{iri} describes itself"));
        let spec = description
            .action_specs()
            .into_iter()
            .find(|a| a.verb == Verb::Source)
            .expect("Source is declared");
        let declared: BTreeSet<String> = spec
            .outputs
            .iter()
            .map(|o| rdf::bare_media_type(o))
            .collect();
        assert_eq!(declared.len(), 1, "{iri}: one face");
        let served = resolve(&kernel, request(Verb::Source, &iri, &args));
        let got = rdf::bare_media_type(&served.repr_type.media_type);
        assert!(
            declared.contains(&got),
            "{iri} served `{got}`, declared {declared:?}"
        );
    }
}

/// The pages are live by DECISION. The strip a page carries is process-global
/// host state — `add_tab` / `hide_tab` — that no golden thread tracks and no
/// cut can reach, so a cached page would keep offering a tab the host has
/// withdrawn. Shown with the exact staleness a cache would introduce: hide a
/// tab between two reads and the second read must not offer it. (This hides
/// `piping` for the rest of this binary; nothing else here asserts on it.)
#[test]
fn pages_are_live_because_the_strip_is_host_state() {
    let kernel = kernel();
    let iri = page_iri("basics");
    let button = "urn:runbook:piping as=text/html";

    let first = resolve(&kernel, source(&iri));
    assert_eq!(first.expiry, Expiry::Always, "a page is live");
    assert!(first.threads().is_empty(), "and carries no thread");
    assert!(text(&first).contains(button), "the strip offers piping");
    assert!(
        !kernel.is_cached(&source(&iri), &Capability::root()),
        "nothing was cached"
    );

    ikigai_runbook::hide_tab("piping");

    let second = resolve(&kernel, source(&iri));
    assert!(
        !text(&second).contains(button),
        "the host withdrew the tab and the next read shows it: a cached page could not"
    );
    // The text face carries the strip too, and is live for the same reason. The
    // JSON face carries no strip (a function of the demo alone) but shares the
    // endpoint's expiry: one endpoint, one decision.
    for face in PAGE_FACES {
        let served = resolve(&kernel, request(Verb::Source, &iri, &[("as", face)]));
        assert_eq!(served.expiry, Expiry::Always, "{iri} as={face}");
    }
}

/// The six constant graphs are `.cacheable()` with an empty thread set — the
/// suite's empty-thread finding, waived by `pure` because a constant has
/// nothing to cut. By hand: `Expiry::Never`, no threads, a cache hit after one
/// read, byte-identical. And the one face the RDF checks are vacuous on — the
/// JSON-LD CONTEXT, zero triples by nature — is held to the contract
/// `urn:jsonld:compact` reads it by.
#[test]
fn the_constant_graphs_are_cached_with_no_thread_by_design() {
    let kernel = kernel();
    for (id, iri, media) in GRAPHS {
        let description = kernel
            .describe(&Iri::parse(iri).expect("a valid IRI"))
            .unwrap_or_else(|| panic!("{iri} describes itself"));
        assert_eq!(
            description.id, id,
            "{iri}: `pure`/`cacheable` are matched by description id (PENDING #57)"
        );

        let first = resolve(&kernel, source(iri));
        assert_eq!(first.expiry, Expiry::Never, "{iri}: a constant");
        assert!(first.threads().is_empty(), "{iri}: nothing to cut");
        assert_eq!(rdf::bare_media_type(&first.repr_type.media_type), media);
        assert!(
            kernel.is_cached(&source(iri), &Capability::root()),
            "{iri}: cached after one read"
        );
        let second = resolve(&kernel, source(iri));
        assert_eq!(first.bytes, second.bytes, "{iri}: byte-identical");
    }

    let context = resolve(&kernel, source("urn:data:ik-context"));
    let json: serde_json::Value =
        serde_json::from_slice(&context.bytes).expect("the context is JSON");
    assert_eq!(
        json["@context"]["@vocab"].as_str(),
        Some(IK),
        "@vocab is the ikigai namespace: {json}"
    );
    let triples = rdf::parse(&context.repr_type.media_type, &context.bytes)
        .expect("a context is valid JSON-LD");
    assert!(
        triples.is_empty(),
        "a context document states no triples, so the RDF checks saw nothing (PENDING #26)"
    );
}

/// The four Turtle faces, by hand and in full: non-empty, no blank nodes (the
/// property shapes are named), every subject under the Account demo's example
/// namespace or the runbook's own `urn:runbook:shape:*`, every predicate and
/// class accounted for — and, beyond what VOCABULARY sees, every term a shape
/// TARGETS (`sh:path`, `sh:targetClass`) is defined too: the `ik:` ones by
/// `ikigai-vocab`, the `ex:` ones by being the demo's.
#[test]
fn the_shapes_are_skolemized_and_reference_only_defined_terms() {
    let kernel = kernel();
    let namespaces = [EXAMPLE_NS.to_string()];
    let mut targeted = BTreeSet::new();
    for (_, iri, media) in GRAPHS.iter().filter(|(_, _, m)| *m == "text/turtle") {
        let repr = resolve(&kernel, source(iri));
        let ttl = text(&repr);
        let triples =
            rdf::parse(media, &repr.bytes).unwrap_or_else(|e| panic!("{iri} parses: {e}\n{ttl}"));
        assert!(!triples.is_empty(), "{iri}: a real graph (PENDING #26)");
        assert!(
            rdf::blank_nodes(&triples).is_empty(),
            "{iri}: skolemized\n{ttl}"
        );
        for triple in &triples {
            let subject = triple.subject.to_string();
            assert!(
                subject.starts_with(&format!("<{EXAMPLE_NS}"))
                    || subject.starts_with(&format!("<{SHAPE_NS}"))
                    || subject.starts_with("<http://xmlns.com/foaf/0.1/"),
                "{iri}: every subject is a demo IRI: {subject}"
            );
            let predicate = triple.predicate.as_str();
            if predicate == SH_PATH || predicate == SH_TARGET_CLASS {
                let object = triple.object.to_string();
                let term = object
                    .strip_prefix('<')
                    .and_then(|o| o.strip_suffix('>'))
                    .unwrap_or_else(|| panic!("{iri}: {predicate} names an IRI: {object}"))
                    .to_string();
                targeted.insert(term);
            }
        }
        for term in rdf::terms(&triples) {
            assert!(
                rdf::is_defined(&term, &namespaces),
                "{iri}: `{term}` is nobody's"
            );
        }
    }
    assert!(
        targeted.contains(&format!("{IK}Endpoint")) && targeted.contains(&format!("{IK}title")),
        "the endpoint shape targets ik:Endpoint and ik:title: {targeted:?}"
    );
    for term in &targeted {
        assert!(
            rdf::is_defined(term, &namespaces),
            "a shape targets `{term}`, which nothing defines (VOCABULARY does not look here)"
        );
    }
}

/// A toy action's required typed input is REQUIRED (PENDING #49, by hand): with
/// every input the action runs; with any one dropped it refuses with the typed
/// `MissingArgument` naming it. The Selection demo's claim — an action is
/// offered only when its required inputs are satisfied — is only honest if the
/// action also refuses to run without them.
#[test]
fn a_toy_action_refuses_a_missing_input() {
    let kernel = kernel();
    for (iri, args) in ACTIONS {
        let full = resolve(&kernel, request(Verb::Source, iri, args));
        let body = text(&full);
        for (_, value) in args {
            assert!(body.contains(value), "{iri}: {body}");
        }
        for (dropped, _) in args {
            let partial: Vec<(&str, &str)> = args
                .iter()
                .copied()
                .filter(|(name, _)| name != dropped)
                .collect();
            match issue(&kernel, request(Verb::Source, iri, &partial)) {
                Err(Error::MissingArgument(name)) => assert_eq!(name, *dropped, "{iri}"),
                other => {
                    panic!("{iri} without `{dropped}`: expected MissingArgument, got {other:?}")
                }
            }
        }
    }
}
