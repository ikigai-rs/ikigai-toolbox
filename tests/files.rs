//! The file family: `urn:toolbox:{name}` over the host's build, its allowlist, its
//! refusals, its errors, and the manifest alias with its Turtle face.

mod common;

use common::Host;
use ikigai_core::Error;

#[test]
fn every_allowlisted_name_is_served_with_the_type_its_name_fixes() {
    let host = Host::new();
    for (name, media) in ikigai_toolbox::FILES {
        let rep = host
            .source(&format!("urn:toolbox:{name}"))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(rep.repr_type.media_type, *media, "{name}");
        assert_eq!(
            rep.bytes,
            common::dist_files()[*name],
            "{name}: the base's bytes, unchanged"
        );
    }
    assert_eq!(
        host.reads(),
        ikigai_toolbox::FILES.len(),
        "one sub-request per file"
    );
}

#[test]
fn the_table_is_the_manifest_generators_table() {
    // tools/manifest.mjs labels each file with the type the crate serves it with;
    // MANIFEST.jsonld itself is the one name the manifest does not describe.
    let script = include_str!("../tools/manifest.mjs");
    let start = script.find("const FILES = {").expect("the table");
    let table = &script[start..start + script[start..].find("\n}").expect("its end")];
    let mut generator: Vec<(String, String)> = table
        .lines()
        .skip(1)
        .map(|line| {
            let mut parts = line
                .split('"')
                .filter(|p| !p.trim().is_empty() && p.trim() != ":" && p.trim() != ",");
            (
                parts.next().unwrap().to_string(),
                parts.next().unwrap().to_string(),
            )
        })
        .collect();
    generator.sort();
    let mut crate_table: Vec<(String, String)> = ikigai_toolbox::FILES
        .iter()
        .filter(|(n, _)| *n != "MANIFEST.jsonld")
        .map(|(n, t)| (n.to_string(), t.to_string()))
        .collect();
    crate_table.sort();
    assert_eq!(generator, crate_table);
}

#[test]
fn a_name_outside_the_allowlist_is_not_found_without_a_sub_request() {
    let host = Host::new();
    for name in [
        "../x",
        "%2e%2e/x",
        "x",
        "Cargo.toml",
        "app.js/../../secret",
        "APP.JS",
        "app.js?x=1",
    ] {
        let iri = format!("urn:toolbox:{name}");
        match host.source(&iri) {
            Err(Error::NotFound(why)) => assert!(why.starts_with(&iri), "{iri}: {why}"),
            other => panic!("{iri}: expected NotFound, got {other:?}"),
        }
    }
    assert_eq!(
        host.reads(),
        0,
        "no name outside the allowlist reached the base"
    );
}

#[test]
fn an_empty_name_matches_no_door() {
    // A URI template's capture is never empty, so `urn:toolbox:` binds nothing: the
    // kernel answers Unresolved, and the base is never asked.
    let host = Host::new();
    match host.source("urn:toolbox:") {
        Err(Error::Unresolved(iri)) => assert_eq!(iri.as_str(), "urn:toolbox:"),
        other => panic!("expected Unresolved, got {other:?}"),
    }
    assert_eq!(host.reads(), 0);
}

#[test]
fn the_bases_not_found_is_not_found_for_the_name_asked() {
    let mut files = common::dist_files();
    files.remove("tokens.css");
    let host = Host::with_files(files);
    match host.source("urn:toolbox:tokens.css") {
        Err(Error::NotFound(why)) => {
            assert!(why.starts_with("urn:toolbox:tokens.css: "), "{why}");
            assert!(
                why.contains("no file `tokens.css`"),
                "the base's reason rides along: {why}"
            );
        }
        other => panic!("expected NotFound, got {other:?}"),
    }
    assert_eq!(host.reads(), 1, "asked once, no fallback");
}

#[test]
fn the_manifest_is_an_alias_and_its_turtle_face_is_a_transreption() {
    let host = Host::new();
    let manifest = host.source("urn:toolbox:manifest").expect("the manifest");
    assert_eq!(manifest.repr_type.media_type, "application/ld+json");
    assert_eq!(manifest.bytes, common::MANIFEST.as_bytes());

    // Its Turtle face, through ikigai-rdf's transreptor (the input reader runs the
    // kernel's plan from application/ld+json to text/turtle).
    let turtle = host.text("urn:toolbox:input:urn:toolbox:manifest", &[]);
    assert!(
        turtle.contains(
            "<https://github.com/abhishekkundu25/turtle-lsp/commit/dcfeed02885c012ad5b7d1c28f33151eda8d75f8>"
        ),
        "the commit: {turtle}"
    );
    let json: Vec<&str> = common::MANIFEST
        .lines()
        .filter_map(|l| l.trim().strip_prefix("\"identifier\": \"sha256-"))
        .collect();
    assert_eq!(
        json.len(),
        ikigai_toolbox::FILES.len() - 1,
        "every file has a digest"
    );
    for digest in json {
        let hex = digest.trim_end_matches(['"', ',']);
        assert!(
            turtle.contains(&format!("sha256-{hex}")),
            "digest {hex} in the Turtle face"
        );
    }
    assert!(!turtle.contains("_:"), "no blank nodes: {turtle}");
}

#[test]
fn the_template_alias_names_the_template_file() {
    let host = Host::new();
    let rep = host.source(ikigai_toolbox::TEMPLATE).expect("the template");
    assert_eq!(rep.repr_type.media_type, "text/html; charset=utf-8");
    assert_eq!(rep.bytes, common::TEMPLATE.as_bytes());
}
