//! The OWL 2 RL pages' steps (Selection's, Two vocabularies, Domain & range, Topics, The
//! thread, Reasoner bounds), run in order through the **real REPL engine** over the modules
//! they name: this crate, `ikigai-reason`, `ikigai-rdf`, `ikigai-sparql` and a cacheable
//! `ikigai-fs` mount for the files the edits write.
//!
//! Like `tests/bytes_demo.rs`, every step is read from the page's own `application/json`
//! face and evaluated as a host would. Unlike it, a page's steps share ONE engine and kernel,
//! because these pages are sequences: a file written by one step is read by the next, and
//! the cache tally (`computed`, `cached`, `2 cached · 1 computed`) is the point of the
//! Thread page. Each page gets a fresh kernel and a fresh file root, so pages do not leak
//! into each other.
//!
//! The kernel's subclass map is built from [`ikigai_runbook::ALIGNMENT_TTL`] with
//! `ikigai_rdf::subclass_axioms`, the way the CLI's embedded host builds it, so the
//! Selection page's "the built map does not" step is held against the same construction.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use futures::executor::block_on;
use ikigai_core::{
    ArgRef, Capability, Description, Fallback, Iri, Kernel, MetaRenderer, ReprType, Representation,
    Request, Space, Verb,
};
use ikigai_engine::{Action, Engine};

struct JsonRenderer;

impl MetaRenderer for JsonRenderer {
    fn render(
        &self,
        description: &Description,
        _target: &ReprType,
    ) -> ikigai_core::Result<Representation> {
        Ok(Representation::new(
            ReprType::new("application/json"),
            serde_json::to_vec(description).expect("serialize description"),
        ))
    }
}

/// A fresh, empty file root for one page's run.
fn file_root(page: &str) -> PathBuf {
    static RUN: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "ikigai-runbook-{page}-{}-{}",
        std::process::id(),
        RUN.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a temporary file root");
    dir
}

fn kernel(spaces: Vec<Arc<dyn Space>>) -> Kernel {
    Kernel::with_meta_renderer(Arc::new(Fallback::new(spaces)), Arc::new(JsonRenderer))
        .with_subclass_axioms(ikigai_rdf::subclass_axioms(ikigai_runbook::ALIGNMENT_TTL))
}

fn host_kernel(root: &PathBuf) -> Kernel {
    kernel(vec![
        Arc::new(ikigai_runbook::space()),
        Arc::new(ikigai_reason::space()),
        Arc::new(ikigai_rdf::space()),
        Arc::new(ikigai_sparql::space()),
        Arc::new(ikigai_fs::cacheable_space(root)),
    ])
}

/// A page's steps as `(label, cmd)`, from its JSON face.
fn steps(page: &str) -> Vec<(String, String)> {
    let request = Request::new(
        Verb::Source,
        Iri::parse(format!("urn:runbook:{page}")).expect("a valid IRI"),
    )
    .with_arg("as", ArgRef::Inline(b"application/json".to_vec()));
    let bare = Kernel::new(Arc::new(ikigai_runbook::space()));
    let page = block_on(bare.issue(request, &Capability::root())).expect("the page");
    let json: serde_json::Value = serde_json::from_slice(&page.bytes).expect("JSON");
    json["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .map(|s| {
            (
                s["label"].as_str().expect("label").to_string(),
                s["cmd"].as_str().expect("cmd").to_string(),
            )
        })
        .collect()
}

/// One step's outcome: the answer, and the engine's cache tally for the line.
struct Ran {
    out: Result<String, String>,
    tally: Option<String>,
}

impl Ran {
    fn ok(&self) -> &str {
        match &self.out {
            Ok(text) => text,
            Err(e) => panic!("expected an answer, got the error {e:?}"),
        }
    }
}

fn eval(engine: &Engine, line: &str) -> Ran {
    match block_on(engine.eval_async(line)) {
        Action::Output(entry) => Ran {
            out: entry.result,
            tally: entry.cache.label(),
        },
        _ => panic!("`{line}` produced no output"),
    }
}

/// Run every step of `page` in order on one engine, checking each with `check`, and print
/// the transcript (`cargo test -- --nocapture` shows it).
fn run_page(page: &str, expected_steps: usize, check: impl Fn(&str, &Ran)) {
    let steps = steps(page);
    assert_eq!(steps.len(), expected_steps, "{page}: {steps:?}");
    let root = file_root(page);
    let engine = Engine::new(host_kernel(&root));
    println!("==== urn:runbook:{page}");
    for (n, (label, cmd)) in steps.iter().enumerate() {
        let ran = eval(&engine, cmd);
        println!(
            "--- {} · {label}\n> {cmd}\n[{}]\n{}",
            n + 1,
            ran.tally.as_deref().unwrap_or("-"),
            match &ran.out {
                Ok(text) => text.clone(),
                Err(e) => format!("error: {e}"),
            }
        );
        check(label, &ran);
    }
    let _ = std::fs::remove_dir_all(root);
}

const RDF_TYPE: &str = "<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>";

/// The page's answer as N-Triples lines, so a check can name one triple exactly.
fn ntriples(turtle: &str) -> Vec<String> {
    let rdf = Arc::new(ikigai_rdf::space());
    let k = Kernel::new(rdf);
    let request = Request::new(
        Verb::Source,
        Iri::parse("urn:rdf:transrept").expect("a valid IRI"),
    )
    .with_arg("content", ArgRef::Inline(turtle.as_bytes().to_vec()))
    .with_arg("from", ArgRef::Inline(b"text/turtle".to_vec()))
    .with_arg("as", ArgRef::Inline(b"application/n-triples".to_vec()));
    let rep = block_on(k.issue(request, &Capability::root())).expect("Turtle → N-Triples");
    String::from_utf8(rep.bytes)
        .expect("UTF-8")
        .lines()
        .map(str::to_string)
        .collect()
}

fn has(turtle: &str, s: &str, p: &str, o: &str) -> bool {
    let line = format!("{s} {p} {o} .");
    ntriples(turtle).contains(&line)
}

fn typed(turtle: &str, who: &str, class: &str) -> bool {
    has(turtle, &format!("<{who}>"), RDF_TYPE, &format!("<{class}>"))
}

const EX: &str = "http://example.org/";
const SCHEMA_PERSON: &str = "https://schema.org/Person";
const FOAF_PERSON: &str = "http://xmlns.com/foaf/0.1/Person";
const OWL_THING: &str = "http://www.w3.org/2002/07/owl#Thing";

fn ex(local: &str) -> String {
    format!("{EX}{local}")
}

/// The `urn:action:*` lines of a `urn:kernel:actions` answer, in order.
fn actions(ran: &Ran) -> Vec<&str> {
    ran.ok()
        .lines()
        .filter(|l| l.starts_with("urn:action:"))
        .collect()
}

fn wrote(ran: &Ran) {
    assert!(ran.ok().starts_with("wrote "), "{:?}", ran.out);
}

#[test]
fn the_selection_page_compares_the_built_map_with_the_reasoner() {
    run_page("selection", 12, |label, ran| match label {
        // A fitted action comes first; input-free resources ride along after it (ikigai-core's
        // ledger #271), so only the actions are compared.
        "have a Person" => assert_eq!(actions(ran), ["urn:action:greet"]),
        "have an Address" => assert_eq!(actions(ran), ["urn:action:geocode"]),
        "have both" => {
            let actions: Vec<&str> = ran.ok().lines().collect();
            for a in ["urn:action:greet", "urn:action:geocode", "urn:action:mail"] {
                assert!(actions.contains(&a), "{actions:?}");
            }
        }
        "run a selected action" => assert_eq!(ran.ok(), "Hello, Ada!"),
        "the alignment graph" => assert!(ran.ok().contains("subClassOf"), "{:?}", ran.out),
        "foaf:Person → greet  (subClassOf!)" => {
            assert!(ran.ok().lines().any(|l| l == "urn:action:greet"))
        }
        "the same question, reasoned" => {
            let out = ran.ok();
            assert!(typed(out, &ex("ada"), SCHEMA_PERSON), "{out}");
            assert!(typed(out, &ex("ada"), OWL_THING), "{out}");
            assert!(!typed(out, &ex("bob"), SCHEMA_PERSON), "{out}");
            assert_eq!(ran.tally.as_deref(), Some("computed"));
        }
        "copy the alignment to a file" | "edit: Customer ⊑ foaf:Person" => wrote(ran),
        "reason over the file" => {
            assert!(typed(ran.ok(), &ex("ada"), SCHEMA_PERSON));
            assert!(!typed(ran.ok(), &ex("bob"), SCHEMA_PERSON));
            assert_eq!(ran.tally.as_deref(), Some("computed"));
        }
        "the reasoner follows" => {
            let out = ran.ok();
            assert!(typed(out, &ex("bob"), FOAF_PERSON), "{out}");
            assert!(typed(out, &ex("bob"), SCHEMA_PERSON), "{out}");
            assert_eq!(
                ran.tally.as_deref(),
                Some("computed"),
                "the cut recomputed it"
            );
        }
        "the built map does not" => {
            assert_eq!(ran.ok(), "", "the built map does not know ex:Customer");
        }
        other => panic!("no expectation for step `{other}`"),
    });
}

#[test]
fn the_reasoner_follows_an_edit_and_a_second_read_is_cached() {
    // Step 9's note says a second run is cached; the page runs it once, so hold it here.
    let root = file_root("selection-cache");
    let engine = Engine::new(host_kernel(&root));
    let line = "source urn:reason:inferred graph=urn:file:alignment.ttl,urn:data:people";
    wrote(&eval(
        &engine,
        "source urn:data:alignment | sink urn:file:alignment.ttl",
    ));
    assert_eq!(eval(&engine, line).tally.as_deref(), Some("computed"));
    assert_eq!(eval(&engine, line).tally.as_deref(), Some("cached"));
    let _ = std::fs::remove_dir_all(root);
}

const SPARQL_HEAD: &str = "who,name";

#[test]
fn two_vocabularies_meet_in_the_closure() {
    run_page("vocabularies", 6, |label, ran| match label {
        "the alignment" | "the data" => assert!(!ran.ok().is_empty()),
        "query the raw data" => {
            let rows: Vec<&str> = ran.ok().lines().map(str::trim_end).collect();
            assert_eq!(
                rows,
                [SPARQL_HEAD, "http://example.org/ada,Ada Lovelace"],
                "only Ada"
            );
        }
        "what the reasoner adds" => {
            let out = ran.ok();
            assert!(typed(out, &ex("grace"), SCHEMA_PERSON), "{out}");
            assert!(typed(out, &ex("ada"), FOAF_PERSON), "{out}");
            assert!(
                has(
                    out,
                    &format!("<{}>", ex("grace")),
                    "<https://schema.org/name>",
                    "\"Grace Hopper\""
                ),
                "{out}"
            );
        }
        "keep the closure" => wrote(ran),
        "the same query over the closure" => {
            let rows: Vec<&str> = ran.ok().lines().map(str::trim_end).collect();
            assert_eq!(
                rows,
                [
                    SPARQL_HEAD,
                    "http://example.org/ada,Ada Lovelace",
                    "http://example.org/grace,Grace Hopper"
                ],
                "both"
            );
        }
        other => panic!("no expectation for step `{other}`"),
    });
}

#[test]
fn domain_and_range_type_an_untyped_entity() {
    run_page("typing", 3, |label, ran| match label {
        "the data" => assert!(!ran.ok().contains(" a "), "nothing is typed: {:?}", ran.out),
        "typed by use" => {
            let out = ran.ok();
            assert!(typed(out, &ex("carol"), SCHEMA_PERSON), "{out}");
            assert!(
                typed(out, &ex("carol-home"), "https://schema.org/PostalAddress"),
                "{out}"
            );
        }
        "select for those types" => {
            let actions: Vec<&str> = ran.ok().lines().collect();
            for a in ["urn:action:greet", "urn:action:geocode", "urn:action:mail"] {
                assert!(actions.contains(&a), "{actions:?}");
            }
        }
        other => panic!("no expectation for step `{other}`"),
    });
}

#[test]
fn the_topic_closure_narrows_the_posts() {
    let topic = |t: &str| format!("<http://example.org/topic/{t}>");
    let bt = "<http://www.w3.org/2004/02/skos/core#broaderTransitive>";
    run_page("topics", 6, |label, ran| match label {
        "the scheme" | "the tagged posts" => assert!(!ran.ok().is_empty()),
        "the chains" => {
            let out = ran.ok();
            for up in ["async", "rust", "programming"] {
                assert!(has(out, &topic("tokio"), bt, &topic(up)), "{out}");
            }
            assert!(!has(out, &topic("sourdough"), bt, &topic("rust")), "{out}");
        }
        "under Rust, raw" => {
            let rows: Vec<&str> = ran.ok().lines().map(str::trim_end).collect();
            assert_eq!(rows, ["post", "http://example.org/post/4"]);
        }
        "keep the closure" => wrote(ran),
        "under Rust, over the closure" => {
            let rows: Vec<&str> = ran.ok().lines().map(str::trim_end).collect();
            assert_eq!(
                rows,
                [
                    "post",
                    "http://example.org/post/1",
                    "http://example.org/post/2",
                    "http://example.org/post/4"
                ]
            );
        }
        other => panic!("no expectation for step `{other}`"),
    });
}

#[test]
fn an_edit_recomputes_only_what_read_it() {
    run_page("thread", 6, |label, ran| match label {
        "1 · write the alignment" | "5 · edit one axiom" => wrote(ran),
        "2 · the two graphs" => {
            assert_eq!(ran.ok(), "urn:file:thread.ttl\nurn:data:topics\n")
        }
        "3 · reason over each" => {
            assert_eq!(ran.tally.as_deref(), Some("1 cached · 2 computed"))
        }
        "4 · again" => assert_eq!(ran.tally.as_deref(), Some("3 cached")),
        "6 · again, after the edit" => {
            assert_eq!(ran.tally.as_deref(), Some("2 cached · 1 computed"));
            assert!(ran
                .ok()
                .contains("<http://example.org/Customer> a <http://www.w3.org/2002/07/owl#Thing> ;\n\t<http://www.w3.org/2000/01/rdf-schema#subClassOf> <https://schema.org/Person>"),
                "{:?}", ran.out);
        }
        other => panic!("no expectation for step `{other}`"),
    });
}

#[test]
fn a_closure_past_a_bound_is_refused_not_truncated() {
    run_page("bounds", 3, |label, ran| match label {
        "a small fan → ok" => {
            let out = ran.ok();
            let typings = ntriples(out)
                .into_iter()
                .filter(|l| l.contains("<http://example.org/Up"))
                .filter(|l| l.contains(RDF_TYPE))
                .count();
            assert_eq!(typings, 12, "{out}");
        }
        "past the output bound → refused" => assert!(
            matches!(&ran.out, Err(e) if e.contains("over budget") && e.contains("closure exceeds 100000 triples")),
            "{:?}",
            ran.out
        ),
        "past the input bound → refused" => assert!(
            matches!(&ran.out, Err(e) if e.contains("over budget") && e.contains("input exceeds 1000 triples")),
            "{:?}",
            ran.out
        ),
        other => panic!("no expectation for step `{other}`"),
    });
}

/// Without `ikigai-reason` the new steps do not resolve: the witness that the tests above
/// exercise the reasoner, and that a host without it should `hide_tab` these pages.
#[test]
fn without_ikigai_reason_the_steps_do_not_resolve() {
    let engine = Engine::new(kernel(vec![
        Arc::new(ikigai_runbook::space()),
        Arc::new(ikigai_rdf::space()),
    ]));
    let ran = eval(&engine, "source urn:reason:inferred graph=urn:data:typing");
    assert!(ran.out.is_err(), "{:?}", ran.out);
}
