//! The Bytes page's steps, run through the **real REPL engine** over the modules they
//! name: this crate, `ikigai-compress` and `ikigai-sniff`.
//!
//! A page is only text in an `hx-get`; the host's `/k/<command>` adapter hands the
//! command to `ikigai-engine`, and pipe routing (which argument a piped value fills)
//! is decided there, not in the kernel. So this file reads every step from the page's
//! own `application/json` face and evaluates its command the way a host does. A step
//! whose command or expectation drifts from what the modules do is a red test, not a
//! button that answers something else in a browser.
//!
//! The kernel carries a JSON meta renderer on purpose: the engine reads each
//! endpoint's contract as `Meta as=application/json`, and without one it fails open
//! and routes every pipe to an input named `in`, which would hide a routing defect.

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

fn kernel() -> Kernel {
    let spaces: Vec<Arc<dyn Space>> = vec![
        Arc::new(ikigai_runbook::space()),
        Arc::new(ikigai_compress::space()),
        Arc::new(ikigai_sniff::space()),
    ];
    Kernel::with_meta_renderer(Arc::new(Fallback::new(spaces)), Arc::new(JsonRenderer))
}

/// The page's steps as `(label, cmd)`, from its JSON face.
fn steps() -> Vec<(String, String)> {
    let request = Request::new(
        Verb::Source,
        Iri::parse("urn:runbook:bytes").expect("a valid IRI"),
    )
    .with_arg("as", ArgRef::Inline(b"application/json".to_vec()));
    let page = block_on(kernel().issue(request, &Capability::root())).expect("the page");
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

fn run(line: &str) -> Result<String, String> {
    match block_on(Engine::new(kernel()).eval_async(line)) {
        Action::Output(entry) => entry.result,
        _ => Err(format!("`{line}` produced no output")),
    }
}

/// What each step must show, by label. Every step of the page is listed: a step added
/// without an expectation fails [`every_step_runs_as_its_note_says`].
fn check(label: &str, out: &Result<String, String>) -> bool {
    match label {
        // The engine refuses to print bytes that are not UTF-8, and says how many.
        "gzip → not text" => matches!(out, Err(e) if e.contains("not UTF-8")),
        "what are these bytes?" => matches!(out, Ok(t) if t.trim() == "application/gzip"),
        "gzip, then gunzip" | "gunzip, unnamed" | "at the bound → ok" => {
            matches!(out, Ok(t) if t.trim_end() == "hello hello hello")
        }
        // `hello hello hello` is 17 bytes: a limit of 17 passes and 16 refuses.
        "past the bound → refused" => {
            matches!(out, Err(e) if e.contains("16-byte limit") && e.contains("max-bytes"))
        }
        _ => false,
    }
}

#[test]
fn every_step_runs_as_its_note_says() {
    let steps = steps();
    assert_eq!(steps.len(), 6, "{steps:?}");
    for (label, cmd) in steps {
        let out = run(&cmd);
        assert!(check(&label, &out), "step `{label}`: `{cmd}` gave {out:?}");
    }
}

/// Without the compress module the page's steps cannot run: the witness that the
/// passing test above is exercising `ikigai-compress`, not something else.
#[test]
fn without_ikigai_compress_the_steps_do_not_resolve() {
    let bare =
        Kernel::with_meta_renderer(Arc::new(ikigai_runbook::space()), Arc::new(JsonRenderer));
    let out = match block_on(Engine::new(bare).eval_async("source urn:compress:gzip hi")) {
        Action::Output(entry) => entry.result,
        _ => panic!("no output"),
    };
    assert!(out.is_err(), "{out:?}");
}
