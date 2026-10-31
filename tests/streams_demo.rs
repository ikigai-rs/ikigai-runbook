//! The Streams page's steps, run in order through the **real REPL engine** over the
//! modules a native host links for them: this crate, `ikigai-rdf` (whose Turtle →
//! N-Triples path answers a one-shot stream) and a cacheable `ikigai-fs` mount (whose
//! file reads answer a seekable body that can be read again), with `ikigai-fs`'s
//! `FileSpill` installed as the kernel's spill facility. The `urn:stream:*` builtins are
//! the kernel's own.
//!
//! Like `tests/bytes_demo.rs`, every step is read from the page's own `application/json`
//! face and evaluated the way a host's `/k/<command>` adapter does, so a step whose
//! command or note drifts from what the kernel does is a red test. One engine runs the
//! whole page: a later step (the cached re-read, `why`, the range after the edit) reads
//! what an earlier one left. The cache tally beside each answer is checked too, because
//! "stored" and "never stored" are what the page is about.

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

/// A fresh, empty directory for one run.
fn fresh_dir(name: &str) -> PathBuf {
    static RUN: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "ikigai-runbook-streams-{name}-{}-{}",
        std::process::id(),
        RUN.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a temporary directory");
    dir
}

/// A native host's kernel for the page, with or without a spill facility.
fn kernel(spill: bool) -> Kernel {
    let spaces: Vec<Arc<dyn Space>> = vec![
        Arc::new(ikigai_runbook::space()),
        Arc::new(ikigai_rdf::space()),
        Arc::new(ikigai_fs::cacheable_space(fresh_dir("files"))),
    ];
    let kernel =
        Kernel::with_meta_renderer(Arc::new(Fallback::new(spaces)), Arc::new(JsonRenderer));
    if spill {
        let facility = ikigai_fs::FileSpill::new(fresh_dir("spill")).expect("a spill facility");
        kernel.with_spill(Arc::new(facility))
    } else {
        kernel
    }
}

/// The page's steps as `(label, cmd)`, from its JSON face.
fn steps() -> Vec<(String, String)> {
    let request = Request::new(
        Verb::Source,
        Iri::parse("urn:runbook:streams").expect("a valid IRI"),
    )
    .with_arg("as", ArgRef::Inline(b"application/json".to_vec()));
    let page = block_on(kernel(false).issue(request, &Capability::root())).expect("the page");
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

/// One line's answer and its cache tally (`computed`, `cached`, `2 uncacheable`, …).
fn run(engine: &Engine, line: &str) -> (Result<String, String>, String) {
    match block_on(engine.eval_async(line)) {
        Action::Output(entry) => (entry.result, entry.cache.label().unwrap_or_default()),
        _ => panic!("`{line}` produced no output"),
    }
}

/// The two triples of the file, as N-Triples, after the edit or before it.
fn n_triples(first: &str) -> String {
    format!(
        "<urn:ex:{first}> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <urn:ex:Person> .\n\
         <urn:ex:alan> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <urn:ex:Person> .\n"
    )
}

/// What each step must show, by label: its answer and its cache tally. Every step of the
/// page is listed: a step added without an expectation fails.
fn check(label: &str, out: &Result<String, String>, cache: &str) -> bool {
    let ada = n_triples("ada");
    match label {
        "1 · write a file" | "11 · edit the file" => {
            matches!(out, Ok(t) if t == "wrote 66 bytes to streams.ttl")
        }
        "2 · read it" => {
            cache == "computed"
                && matches!(out, Ok(t) if t.starts_with("<urn:ex:ada> a <urn:ex:Person> ."))
        }
        "3 · read it again" => cache == "cached" && out.as_ref().is_ok_and(|t| t.len() == 66),
        "4 · a byte range" => cache == "computed" && matches!(out, Ok(t) if t == "<urn:ex:ada>"),
        "5 · past the end → refused" => {
            matches!(out, Err(e) if e.contains("past the end") && e.contains("(66 bytes)"))
        }
        // Computed both times: the file under it is cached, so nothing but the one-shot
        // body keeps the answer from being stored.
        "6 · a one-shot stream" | "7 · and again" => {
            cache == "computed" && matches!(out, Ok(t) if *t == ada)
        }
        // The one reason logged, twice: no dependency of the file's, no other cause.
        "8 · why not stored?" => matches!(out, Ok(t) if t
            .lines()
            .filter(|l| l.trim_start().starts_with("uncached"))
            .eq(["  uncached    ×2  one-shot body  [root]"])),
        "9 · collect it" | "10 · spill it" => matches!(out, Ok(t) if *t == ada),
        "12 · the range follows" => {
            cache == "computed" && matches!(out, Ok(t) if t == "<urn:ex:bob>")
        }
        _ => false,
    }
}

#[test]
fn every_step_runs_as_its_note_says() {
    let steps = steps();
    assert_eq!(steps.len(), 12, "{steps:?}");
    let engine = Engine::new(kernel(true));
    for (label, cmd) in steps {
        let (out, cache) = run(&engine, &cmd);
        println!("{label}\n  > {cmd}\n  {out:?} [{cache}]");
        assert!(
            check(&label, &out, &cache),
            "step `{label}`: `{cmd}` gave {out:?} [{cache}]"
        );
    }
}

/// The range step re-run before the edit is a cache hit: what step 12 shows computed is
/// the edit's cut, not a range that is never stored.
#[test]
fn a_range_is_stored_until_the_file_is_written() {
    let steps = steps();
    let cmd = |label: &str| {
        steps
            .iter()
            .find(|(l, _)| l == label)
            .map(|(_, c)| c.clone())
            .expect("the step")
    };
    let engine = Engine::new(kernel(false));
    assert!(run(&engine, &cmd("1 · write a file")).0.is_ok());
    let range = cmd("4 · a byte range");
    assert_eq!(run(&engine, &range).1, "computed");
    assert_eq!(run(&engine, &range).1, "cached");
}

/// The spill step's note: a host with no spill facility refuses it by name, rather than
/// buffer the stream instead.
#[test]
fn without_a_spill_facility_the_spill_step_is_refused_by_name() {
    let steps = steps();
    let cmd = |label: &str| {
        steps
            .iter()
            .find(|(l, _)| l == label)
            .map(|(_, c)| c.clone())
            .expect("the step")
    };
    let spill = cmd("10 · spill it");
    let engine = Engine::new(kernel(false));
    assert!(run(&engine, &cmd("1 · write a file")).0.is_ok());
    let (out, _) = run(&engine, &spill);
    assert!(
        matches!(&out, Err(e) if e.contains("no spill facility")),
        "{out:?}"
    );
}
