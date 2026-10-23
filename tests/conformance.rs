//! The module recipe as one test: ikigai-conformance walks every endpoint the test
//! host binds (the toolbox's, and the host's fixtures beside them) and reports every
//! violation at once.
//!
//! - Every toolbox read is declared `cacheable`: the file and input readers mark
//!   their answers cacheable and take expiry and threads from the sub-request, the
//!   views are compositions (cacheable while every part is), `echo`,
//!   `urn:toolbox:none` and `urn:toolbox:path:{rest}` are pure functions of their
//!   name (and, for the path, its `base=`).
//! - FAILURES is opted into (`check_failures`, ahead of 0.11 making it the default).
//!   No toolbox action states its failure classes, deliberately: the readers pass
//!   their sub-request's failure through, and what a host binds at the base or names
//!   as an input can fail in any class, so a stated list would be a guess. The report
//!   lists them on its `failures:` line.
//! - `space()` is self-named `urn:iki:space:toolbox` (it reads its build at the fixed
//!   name `ikigai_toolbox::DIST`); `space_at(base)` is host-named.

mod common;

use std::sync::Arc;

use common::Host;
use ikigai_conformance::{Check, Fixture, Suite};
use ikigai_core::Verb;

/// Every endpoint the toolbox binds, by description id.
const TOOLBOX: [&str; 7] = [
    "toolbox-file",
    "toolbox-input",
    "toolbox-page",
    "toolbox-check",
    "echo",
    "toolbox-none",
    "toolbox-path",
];

const RDF_FACE_IS_THE_NAMES: &str = "the media type is fixed by the file NAME, not by as=: \
     only MANIFEST.jsonld has an RDF face (tests/files.rs checks its Turtle face)";

#[test]
fn conforms() {
    let host = Host::new();
    let space_at = Arc::new(ikigai_toolbox::space_at("urn:example:elsewhere:"));
    let report = Suite::new()
        .check_failures()
        .fixture(Fixture::new("toolbox-file", Verb::Source).binding("name", "style.css"))
        .fixture(Fixture::new("toolbox-input", Verb::Source).binding("iri", "urn:example:data"))
        .fixture(Fixture::new("toolbox-check", Verb::Source).binding("iri", "urn:example:data"))
        .fixture(Fixture::new("echo", Verb::Source).binding("message", "auto"))
        .fixture(Fixture::new("test-dist", Verb::Source).binding("path", "style.css"))
        .cacheable("toolbox-file")
        .cacheable("toolbox-input")
        .cacheable("toolbox-page")
        .cacheable("toolbox-check")
        .pure("echo")
        .pure("toolbox-none")
        .cacheable("toolbox-none")
        .pure("toolbox-path")
        .cacheable("toolbox-path")
        // The two RDF checks ask ONE fixture for every declared RDF face by `as=`; here
        // the media type is the NAME's, so the fixture (style.css) answers text/css under
        // every `as=`, and the JSON-LD face belongs to MANIFEST.jsonld alone. Its Turtle
        // face is held to no blank nodes by tests/files.rs instead.
        .opt_out_check("toolbox-file", Check::SkolemRdf, RDF_FACE_IS_THE_NAMES)
        .opt_out_check("toolbox-file", Check::Vocabulary, RDF_FACE_IS_THE_NAMES)
        // The host's fixtures: constants, so pure; their example.org terms are fixtures'.
        .pure("test-dist")
        .pure("test-data")
        .pure("test-shapes")
        .pure("test-secret")
        .pure("test-jsonld")
        .namespace("https://example.org/")
        // ikigai-rdf's own endpoints, here only as the manifest's transreptor: they
        // conform in their own repo, and a fixture for each would test that crate.
        .opt_out(
            "rdf-union",
            None,
            "ikigai-rdf's, held to the recipe in its own repo",
        )
        .opt_out(
            "rdf-diff",
            None,
            "ikigai-rdf's, held to the recipe in its own repo",
        )
        .opt_out(
            "rdf-transrept",
            None,
            "ikigai-rdf's, held to the recipe in its own repo",
        )
        .self_named_space("toolbox", ikigai_toolbox::space)
        .host_named_space("ikigai_toolbox::space_at(base)", space_at)
        .run_blocking(&host.kernel);
    assert!(report.is_clean(), "{report}");
    println!("{report}");
    assert_eq!(
        ikigai_core::space_iri("toolbox").as_str(),
        ikigai_toolbox::SPACE_ID
    );
    for id in TOOLBOX {
        assert!(
            report.to_string().contains(id),
            "the walk saw `{id}`: {report}"
        );
    }
}
