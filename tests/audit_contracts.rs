//! Audit boundaries not covered by the general conformance walk: unusable optional
//! values and engine routing to each toy action with a single required input.

use std::sync::Arc;

use futures::executor::block_on;
use ikigai_core::{
    ArgRef, Capability, Description, Iri, Kernel, MetaRenderer, ReprType, Representation, Request,
    Verb,
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
    Kernel::with_meta_renderer(Arc::new(ikigai_runbook::space()), Arc::new(JsonRenderer))
}

#[test]
fn unusable_optional_faces_are_typed_refusals() {
    for bytes in [
        vec![],
        b"text/plain\r\nunknown".to_vec(),
        "日本語".as_bytes().to_vec(),
        vec![b'x'; 100_000],
        vec![255],
    ] {
        let req = Request::new(Verb::Source, Iri::parse("urn:runbook:basics").unwrap())
            .with_arg("as", ArgRef::Inline(bytes));
        assert!(
            matches!(block_on(kernel().issue(req, &Capability::root())), Err(ikigai_core::Error::InvalidArgument { name, .. }) if name == "as")
        );
    }
}

#[test]
fn a_pipe_reaches_the_single_required_action_input() {
    for (line, expected) in [
        (
            "source urn:action:greet who=Ada | urn:action:greet",
            "Hello, Hello, Ada!!",
        ),
        (
            "source urn:action:greet who=Ada | urn:action:geocode",
            "Geocoded: Hello, Ada!",
        ),
    ] {
        match block_on(Engine::new(kernel()).eval_async(line)) {
            Action::Output(entry) => assert_eq!(entry.result.unwrap().trim_end(), expected),
            _ => panic!("no output"),
        }
    }
}
