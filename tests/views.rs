//! The views: the page and the check page as compositions of the template, their
//! snapshots (UI conventions D18, ahead of the shared kit), what they escape, and the
//! input reader's authority.
//!
//! `UPDATE_SNAPSHOTS=1 cargo test --test views` rewrites the snapshots; read the diff.

mod common;

use common::Host;
use ikigai_core::{Capability, Error};

fn snapshot(name: &str, actual: &str) {
    let path = format!("{}/tests/snapshots/{name}", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(&path, actual).expect("write the snapshot");
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{path}: {e} (UPDATE_SNAPSHOTS=1 writes it)"));
    assert_eq!(actual, expected, "{name} differs from its snapshot");
}

#[test]
fn the_empty_page_snapshot() {
    let host = Host::new();
    let page = host.source("urn:toolbox:page").expect("the page");
    assert_eq!(page.repr_type.media_type, "text/html; charset=utf-8");
    let html = String::from_utf8(page.bytes).unwrap();
    snapshot("page.html", &html);
    // About itself, auto theme, no inputs (the page reads both empty entries as none).
    assert!(html.contains(r#"<html lang="en" data-theme="auto">"#));
    assert!(html.contains(r#"about="urn:toolbox:page" data-inputs="[&quot;&quot;,&quot;&quot;]""#));
}

#[test]
fn the_check_page_snapshot() {
    let host = Host::new();
    let html = host.text(
        "urn:toolbox:check:urn:example:data",
        &[("shapes", "urn:example:shapes"), ("theme", "dark")],
    );
    snapshot("check.html", &html);
    assert!(html.contains(r#"data-theme="dark""#));
    // RDFa: the page is about the resource it checks, and names it as text: a link to
    // the input's path would be one a static host cannot serve (ledger #2106).
    assert!(html.contains(r#"about="urn:example:data""#));
    assert!(html.contains(r#"<span class="checked">urn:example:data</span>"#));
    assert!(!html.contains(r#"href="/toolbox/input/"#), "{html}");
    // The inputs: same-origin, path-absolute, one percent-encoded segment each.
    assert!(html.contains(
        r#"data-inputs="[&quot;/toolbox/input/urn%3Aexample%3Adata&quot;,&quot;/toolbox/input/urn%3Aexample%3Ashapes&quot;]""#
    ));
}

#[test]
fn without_shapes_the_check_page_names_one_input() {
    let html = Host::new().text("urn:toolbox:check:urn:example:data", &[]);
    assert!(html.contains(
        r#"data-inputs="[&quot;/toolbox/input/urn%3Aexample%3Adata&quot;,&quot;&quot;]""#
    ));
    assert!(html.contains(r#"data-theme="auto""#));
}

#[test]
fn a_hostile_theme_comes_out_escaped() {
    let hostile = "\"><script>alert(1)</script><x a='\u{1}";
    let html = Host::new().text("urn:toolbox:page", &[("theme", hostile)]);
    assert!(!html.contains("<script>alert"), "{html}");
    assert!(html
        .contains(r#"data-theme="&quot;&gt;&lt;script&gt;alert(1)&lt;/script&gt;&lt;x a=&#39;"#));
    assert!(html.contains("\u{FFFD}"), "a C0 control becomes U+FFFD");
}

#[test]
fn a_hostile_iri_comes_out_escaped_and_encoded() {
    // The most hostile IRI the kernel accepts: quotes, ampersand, slash, query and
    // fragment characters. Each place it lands has its own rule: element text and
    // attributes are HTML-escaped, a path segment is percent-encoded.
    let host = Host::new();
    let iri = "urn:example:a'b&c/../d?e=f";
    let html = host.text(
        &format!("urn:toolbox:check:{iri}"),
        &[("shapes", "urn:x:\"'><")],
    );
    assert!(
        html.contains(r#"about="urn:example:a&#39;b&amp;c/../d?e=f""#),
        "{html}"
    );
    assert!(html.contains(">urn:example:a&#39;b&amp;c/../d?e=f</span>"));
    assert!(html.contains("/toolbox/input/urn%3Aexample%3Aa%27b%26c%2F..%2Fd%3Fe%3Df"));
    assert!(
        html.contains("/toolbox/input/urn%3Ax%3A%22%27%3E%3C"),
        "{html}"
    );
    for raw in ["'b&c", "\"'><"] {
        assert!(!html.contains(raw), "{raw} reached the page unescaped");
    }
}

#[test]
fn an_input_is_read_under_the_callers_capability() {
    let host = Host::new();
    let none = Capability::root().attenuate(Vec::<String>::new());
    match host.source_with("urn:toolbox:input:urn:example:secret", &[], &none) {
        Err(Error::Denied(_)) => {}
        other => panic!("a caller who cannot read it is refused, got {other:?}"),
    }
    let granted = Capability::root().attenuate([common::SECRET_SCOPE]);
    let rep = host
        .source_with("urn:toolbox:input:urn:example:secret", &[], &granted)
        .expect("a caller who can read it sees it");
    assert_eq!(rep.repr_type.media_type, "text/plain; charset=utf-8");
    assert_eq!(rep.bytes, common::DATA.as_bytes());
}

#[test]
fn an_inputs_turtle_face_is_reached_by_transreption() {
    let text = Host::new().text("urn:toolbox:input:urn:example:jsonld", &[]);
    assert!(text.contains("<https://example.org/toolbox>"), "{text}");
    assert!(text.contains("<https://example.org/jsonld>"), "{text}");
}

#[test]
fn an_input_that_is_not_an_absolute_iri_is_refused() {
    match Host::new().source("urn:toolbox:input:no-scheme") {
        Err(Error::InvalidArgument { name, .. }) => assert_eq!(name, "iri"),
        other => panic!("expected InvalidArgument on `iri`, got {other:?}"),
    }
}

#[test]
fn a_check_page_does_not_link_the_sample_editor() {
    // Upstream's hosted editor opens its own sample, not the resource checked here, and
    // nothing checked here may go to it: a check page has no link to it (ledger #2106).
    let host = Host::new();
    for (name, args) in [
        (
            "urn:toolbox:check:urn:example:data",
            &[("shapes", "urn:example:shapes")][..],
        ),
        ("urn:toolbox:page", &[][..]),
    ] {
        let html = host.text(name, args);
        assert!(!html.contains("ed-link"), "{name}: {html}");
        assert!(
            !html.contains("https://"),
            "{name} links off the host: {html}"
        );
    }
}

#[test]
fn the_file_inputs_are_named() {
    let html = Host::new().text("urn:toolbox:check:urn:example:data", &[]);
    assert!(html.contains(r#"id="picker" class="hidden-input" aria-label="Files to open""#));
    assert!(
        html.contains(r#"id="folder-picker" class="hidden-input" aria-label="A folder to open""#)
    );
}

#[test]
fn base_puts_every_path_under_the_hosts_prefix() {
    // A host serving the family below a prefix (a static site under a project path)
    // passes it as base=, and the page names its assets and inputs under it.
    let base = "/ikigai-tutorial/toolbox/";
    let html = Host::new().text(
        "urn:toolbox:check:urn:example:data",
        &[("shapes", "urn:example:shapes"), ("base", base)],
    );
    for path in ["style.css", "tokens.css", "app.js"] {
        assert!(
            html.contains(&format!(r#"="{base}{path}""#)),
            "{path}: {html}"
        );
    }
    assert!(html.contains(&format!(
        r#"data-inputs="[&quot;{base}input/urn%3Aexample%3Adata&quot;,&quot;{base}input/urn%3Aexample%3Ashapes&quot;]""#
    )));
    assert!(
        !html.contains(r#""/toolbox/"#),
        "a path outside the base: {html}"
    );
    // The empty page takes it too, and still names no input.
    let page = Host::new().text("urn:toolbox:page", &[("base", base)]);
    assert!(
        page.contains(&format!(r#"href="{base}style.css""#)),
        "{page}"
    );
    assert!(page.contains(r#"data-inputs="[&quot;&quot;,&quot;&quot;]""#));
}

#[test]
fn a_base_that_is_not_a_same_origin_path_is_refused() {
    let host = Host::new();
    for bad in [
        "//evil.example/",
        "/\\evil.example/",
        "https://evil.example/",
        "javascript:alert(1)//",
        "toolbox/",
        "/toolbox",
        "/a b/",
        "/a\"b/",
        "/a?b/",
        "",
    ] {
        for name in ["urn:toolbox:page", "urn:toolbox:check:urn:example:data"] {
            match host.source_with(name, &[("base", bad)], &Capability::root()) {
                Err(Error::InvalidArgument { name: arg, .. }) => assert_eq!(arg, "base", "{bad:?}"),
                other => {
                    panic!("{name} with base={bad:?}: expected InvalidArgument, got {other:?}")
                }
            }
        }
    }
}
