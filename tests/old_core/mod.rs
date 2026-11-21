//! The bridge from an engine test's core 0.3 kernel to this crate's core 0.4 space.
//!
//! `ikigai-engine` and the modules the pages' steps name (`ikigai-compress`, `ikigai-sniff`,
//! `ikigai-reason`, `ikigai-rdf`, `ikigai-sparql`, `ikigai-fs`) are tested here on their core
//! 0.3 releases, so a test of this crate through the engine has two cores. The engine and those
//! modules run in a core 0.3 kernel, and every binding of [`ikigai_runbook::space`] is bound
//! there too, forwarding to a real core 0.4 kernel over that space. What crosses is the request
//! (verb, target and inline arguments; the old kernel has already read any reference the
//! endpoint's contract declares), each endpoint's [`Description`](ikigai_core::Description) as
//! the JSON both cores share, and the answer's bytes, media type and expiry. A caller's
//! capability does not cross: every test here issues as root, and this crate's access rules
//! are pinned by the tests on core 0.4 (`tests/conformance.rs`, the unit tests). See the
//! manifest for when this goes.

use std::sync::Arc;

use ikigai_core::Space as _;
use ikigai_core_for_old_modules as old;

/// Renders a core 0.4 description as the JSON the old kernel reads back.
struct NewJsonRenderer;

impl ikigai_core::MetaRenderer for NewJsonRenderer {
    fn render(
        &self,
        description: &ikigai_core::Description,
        _target: &ikigai_core::ReprType,
    ) -> ikigai_core::Result<ikigai_core::Representation> {
        Ok(ikigai_core::Representation::new(
            ikigai_core::ReprType::new("application/json"),
            serde_json::to_vec(description).expect("serialize description"),
        ))
    }
}

/// Every binding of [`ikigai_runbook::space`], under the same patterns, endpoint names and
/// descriptions, named [`ikigai_runbook::SPACE_ID`] as the space names itself, each forwarding
/// to a core 0.4 kernel over it.
pub fn runbook_space() -> old::EndpointSpace {
    let new = Arc::new(ikigai_core::Kernel::with_meta_renderer(
        Arc::new(ikigai_runbook::space()),
        Arc::new(NewJsonRenderer),
    ));
    let entries = ikigai_runbook::space()
        .entries()
        .expect("the runbook space enumerates its bindings");
    let mut space = old::EndpointSpace::new();
    for entry in entries {
        let target = ikigai_core::Iri::parse(entry.pattern.as_str()).expect("an exact pattern");
        let meta = futures::executor::block_on(new.issue(
            ikigai_core::Request::new(ikigai_core::Verb::Meta, target).with_arg(
                "as",
                ikigai_core::ArgRef::Inline(b"application/json".to_vec()),
            ),
            &ikigai_core::Capability::root(),
        ))
        .expect("every runbook binding describes itself");
        let description: old::Description =
            serde_json::from_slice(&meta.bytes).expect("a description both cores read");
        space = space.bind(
            old::Exact::new(entry.pattern.clone()),
            RunbookBridge {
                kernel: new.clone(),
                name: entry.endpoint,
                description,
            },
        );
    }
    space.named(old::Iri::parse(ikigai_runbook::SPACE_ID).expect("the space IRI"))
}

struct RunbookBridge {
    kernel: Arc<ikigai_core::Kernel>,
    name: String,
    description: old::Description,
}

/// The request as core 0.4 spells it: same verb and target, inline arguments.
fn request(inv: &old::Invocation<'_>) -> old::Result<ikigai_core::Request> {
    let verb: ikigai_core::Verb =
        serde_json::from_value(serde_json::to_value(inv.request.verb).expect("serialize verb"))
            .map_err(|e| old::Error::Endpoint(format!("a verb core 0.4 lacks: {e}")))?;
    let target = ikigai_core::Iri::parse(inv.request.target.as_str())
        .map_err(|e| old::Error::Endpoint(e.to_string()))?;
    let mut request = ikigai_core::Request::new(verb, target);
    for (name, arg) in &inv.request.args {
        let old::ArgRef::Inline(value) = arg else {
            return Err(old::Error::Endpoint(format!(
                "the bridge only carries inline arguments, not `{name}`"
            )));
        };
        request = request.with_arg(name, ikigai_core::ArgRef::Inline(value.clone()));
    }
    Ok(request)
}

fn error(new: ikigai_core::Error) -> old::Error {
    match new {
        ikigai_core::Error::NotFound(detail) => old::Error::NotFound(detail),
        ikigai_core::Error::Denied(denial) => old::Error::Denied(denial.message().to_string()),
        ikigai_core::Error::InvalidArgument { name, detail } => {
            old::Error::InvalidArgument { name, detail }
        }
        ikigai_core::Error::MissingArgument(name) => old::Error::MissingArgument(name),
        other => old::Error::Endpoint(other.to_string()),
    }
}

#[async_trait::async_trait]
impl old::Endpoint for RunbookBridge {
    async fn invoke(&self, inv: &old::Invocation<'_>) -> old::Result<old::Representation> {
        let answer = self
            .kernel
            .issue(request(inv)?, &ikigai_core::Capability::root())
            .await
            .map_err(error)?;
        let mut repr_type = old::ReprType::new(answer.repr_type.media_type.as_str());
        for (key, value) in &answer.repr_type.params {
            repr_type = repr_type.with_param(key, value);
        }
        let mut out = old::Representation::new(repr_type, answer.bytes);
        out.expiry =
            serde_json::from_value(serde_json::to_value(answer.expiry).expect("serialize expiry"))
                .expect("an expiry both cores read");
        Ok(out)
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn describe(&self) -> old::Description {
        self.description.clone()
    }
}
