//! A test host for the toolbox: the build directory as an in-memory space at
//! `urn:iki:toolbox:dist:{path}` that counts its sub-requests, plus a few resources
//! a page can be asked to check. Each fake describes itself fully, because the
//! conformance walk holds every endpoint the kernel binds to the recipe.
#![allow(dead_code)] // each test binary uses its own part of this module

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use futures::executor::block_on;
use ikigai_core::{
    ArgRef, ArgSpec, Capability, Description, EndpointSpace, Error, Exact, Fallback, FnEndpoint,
    Invocation, Iri, Kernel, ReprType, Representation, Request, Result, Space, UriTemplate, Verb,
};

/// The page template as `build.sh` writes it (the smoke test holds this copy equal
/// to `dist/page.template.html`).
pub const TEMPLATE: &str = include_str!("../fixtures/page.template.html");
/// A manifest as `build.sh` writes it (the smoke test holds its shape to a build's).
pub const MANIFEST: &str = include_str!("../fixtures/MANIFEST.jsonld");

/// The Turtle every check fixture reads.
pub const DATA: &str = "@prefix ex: <https://example.org/> .\nex:toolbox ex:checks ex:turtle .\n";
pub const SHAPES: &str = "@prefix sh: <http://www.w3.org/ns/shacl#> .\n\
     <urn:example:shape> a sh:NodeShape ; sh:targetClass <https://example.org/Thing> ; sh:closed true .\n";
/// The scope `urn:example:secret` requires: a caller without it is refused.
pub const SECRET_SCOPE: &str = "urn:cap:example:secret";

/// The build directory's files: every allowlisted name, with recognizable bytes.
pub fn dist_files() -> BTreeMap<String, Vec<u8>> {
    ikigai_toolbox::FILES
        .iter()
        .map(|(name, _)| {
            let bytes = match *name {
                "page.template.html" => TEMPLATE.as_bytes().to_vec(),
                "MANIFEST.jsonld" => MANIFEST.as_bytes().to_vec(),
                other => format!("the bytes of {other}").into_bytes(),
            };
            (name.to_string(), bytes)
        })
        .collect()
}

fn turtle(id: &str, title: &str, body: &'static str) -> FnEndpoint {
    FnEndpoint::new(id, move |_: &Invocation<'_>| {
        Ok(Representation::new(ReprType::new("text/turtle"), body.as_bytes().to_vec()).cacheable())
    })
    .with_description(
        Description::new(id)
            .title(title)
            .summary("A fixture resource for the toolbox's tests.")
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .output("text/turtle"),
    )
}

/// A host: the build directory (counting reads), three Turtle resources (one of
/// them guarded), the manifest's JSON-LD transreptor (ikigai-rdf), and the toolbox.
pub struct Host {
    pub kernel: Kernel,
    pub space: Arc<dyn Space>,
    pub reads: Arc<AtomicUsize>,
}

impl Host {
    pub fn new() -> Host {
        Host::with_files(dist_files())
    }

    pub fn with_files(files: BTreeMap<String, Vec<u8>>) -> Host {
        Host::with_toolbox(files, Arc::new(ikigai_toolbox::space()))
    }

    pub fn with_toolbox(files: BTreeMap<String, Vec<u8>>, toolbox: Arc<dyn Space>) -> Host {
        let reads = Arc::new(AtomicUsize::new(0));
        let counter = reads.clone();
        let dist = FnEndpoint::new("test-dist", move |inv: &Invocation<'_>| {
            counter.fetch_add(1, Ordering::SeqCst);
            let path = inv.bindings.get("path").unwrap_or_default();
            match files.get(path) {
                Some(bytes) => Ok(Representation::new(
                    ReprType::new("application/octet-stream"),
                    bytes.clone(),
                )
                .cacheable()),
                None => Err(Error::NotFound(format!("no file `{path}` in the build"))),
            }
        })
        .with_description(
            Description::new("test-dist")
                .title("The build directory")
                .summary("Stands in for ikigai-fs over the toolbox's dist/.")
                .verb(Verb::Source)
                .verb(Verb::Meta)
                .input(
                    ArgSpec::new("path")
                        .class("http://www.w3.org/2001/XMLSchema#string")
                        .binding(),
                )
                .output("application/octet-stream"),
        );
        let secret = FnEndpoint::new("test-secret", |inv: &Invocation<'_>| {
            if !inv.capability.allows(SECRET_SCOPE) {
                return Err(Error::Denied(format!("needs {SECRET_SCOPE}")));
            }
            Ok(
                Representation::new(ReprType::new("text/turtle"), DATA.as_bytes().to_vec())
                    .cacheable(),
            )
        })
        .with_description(
            Description::new("test-secret")
                .title("A guarded resource")
                .summary("Turtle only a caller holding urn:cap:example:secret may read.")
                .verb(Verb::Source)
                .verb(Verb::Meta)
                .requires(SECRET_SCOPE)
                .output("text/turtle"),
        );
        let jsonld = FnEndpoint::new("test-jsonld", |_: &Invocation<'_>| {
            Ok(Representation::new(
                ReprType::new("application/ld+json"),
                br#"{"@id": "https://example.org/toolbox", "https://example.org/checks": {"@id": "https://example.org/jsonld"}}"#.to_vec(),
            )
            .cacheable())
        })
        .with_description(
            Description::new("test-jsonld")
                .title("A JSON-LD resource")
                .summary("A fixture whose Turtle face is a transreption.")
                .verb(Verb::Source)
                .verb(Verb::Meta)
                .output("application/ld+json"),
        );
        let host = EndpointSpace::new()
            .bind(
                UriTemplate::parse("urn:iki:toolbox:dist:{path}").expect("template"),
                dist,
            )
            .bind(
                Exact::new("urn:example:data"),
                turtle("test-data", "Data", DATA),
            )
            .bind(
                Exact::new("urn:example:shapes"),
                turtle("test-shapes", "Shapes", SHAPES),
            )
            .bind(Exact::new("urn:example:secret"), secret)
            .bind(Exact::new("urn:example:jsonld"), jsonld);
        let space: Arc<dyn Space> = Arc::new(Fallback::new(vec![
            toolbox,
            Arc::new(host),
            Arc::new(ikigai_rdf::space()),
        ]));
        let kernel = Kernel::new(space.clone());
        Host {
            kernel,
            space,
            reads,
        }
    }

    pub fn reads(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }

    pub fn source(&self, iri: &str) -> Result<Representation> {
        self.source_with(iri, &[], &Capability::root())
    }

    pub fn source_with(
        &self,
        iri: &str,
        args: &[(&str, &str)],
        capability: &Capability,
    ) -> Result<Representation> {
        let mut request = Request::new(Verb::Source, Iri::parse(iri).expect("a valid IRI"));
        for (name, value) in args {
            request = request.with_arg(*name, ArgRef::Inline(value.as_bytes().to_vec()));
        }
        block_on(self.kernel.issue(request, capability))
    }

    pub fn text(&self, iri: &str, args: &[(&str, &str)]) -> String {
        let rep = self
            .source_with(iri, args, &Capability::root())
            .unwrap_or_else(|e| panic!("{iri}: {e}"));
        String::from_utf8(rep.bytes).expect("UTF-8")
    }
}
