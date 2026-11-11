//! ikigai-fn 0.8's change to compose (ledger #2506): an unquoted `<iri>` marker argument
//! is issued as a reference, which the kernel settles by the target's contract. The page
//! template writes none, so what the views answer did not move; these tests hold that,
//! say what such a marker would do, and hold the check page to its caller's authority.

mod common;

use common::{dist_files, Host, DATA, SECRET_SCOPE, TEMPLATE};
use futures::executor::block_on;
use ikigai_core::{ArgRef, Capability, Error, Iri, Request, Verb};

fn no_scopes() -> Capability {
    Capability::root().attenuate(Vec::<String>::new())
}

fn granted() -> Capability {
    Capability::root().attenuate([SECRET_SCOPE])
}

/// A host whose page template is `template`.
fn host_with_template(template: &str) -> Host {
    let mut files = dist_files();
    files.insert(
        "page.template.html".to_string(),
        template.as_bytes().to_vec(),
    );
    Host::with_files(files)
}

/// The input names a composed check page lists, as the route rule reads them back:
/// `/toolbox/input/{iri}` is `urn:toolbox:input:{iri}`, the IRI still one encoded segment.
fn input_names(html: &str) -> Vec<String> {
    let (_, rest) = html.split_once("data-inputs=\"[").expect("data-inputs");
    let (list, _) = rest.split_once("]\"").expect("data-inputs closes");
    list.split(',')
        .map(|entry| entry.replace("&quot;", ""))
        .filter(|path| !path.is_empty())
        .map(|path| {
            let segment = path
                .strip_prefix("/toolbox/input/")
                .expect("under the base");
            let iri = ikigai_fn::html::percent_decode(segment, false).expect("one encoded segment");
            format!("urn:toolbox:input:{iri}")
        })
        .collect()
}

#[test]
fn the_template_writes_no_reference_argument() {
    // `name=<…>` is the only spelling compose 0.8 reads differently. Were one added, the
    // kernel would read its referent for a by-value input: decide that on purpose.
    assert!(
        !TEMPLATE.contains("=<"),
        "the page template gained a reference argument"
    );
}

#[test]
fn a_reference_marker_reads_under_the_callers_capability() {
    // What changed, through the toolbox's own door: `base` is a by-value input, so the
    // kernel reads the referent, as the caller. Under 0.7 this was the inline text
    // `<urn:example:secret>`, refused as InvalidArgument whoever asked.
    let host = host_with_template("$h{urn:toolbox:path:x?base=<urn:example:secret>}");
    match host.source_with("urn:toolbox:page", &[], &no_scopes()) {
        Err(Error::Denied(_)) => {}
        other => panic!("a caller who cannot read the referent is refused, got {other:?}"),
    }
    // A caller who can read it gets past the read, and `base` refuses what it holds.
    match host.source_with("urn:toolbox:page", &[], &granted()) {
        Err(Error::InvalidArgument { name, .. }) => assert_eq!(name, "base"),
        other => panic!("the referent is not a base, got {other:?}"),
    }
}

#[test]
fn a_quoted_angle_value_stays_inline_text() {
    let host = host_with_template(r#"$h{urn:toolbox:path:x?base="<urn:example:secret>"}"#);
    match host.source_with("urn:toolbox:page", &[], &no_scopes()) {
        Err(Error::InvalidArgument { name, detail }) => {
            assert_eq!(name, "base");
            assert!(detail.contains("<urn:example:secret>"), "{detail}");
        }
        other => panic!("expected the text refused as a base, got {other:?}"),
    }
}

#[test]
fn an_angle_valued_request_argument_stays_a_value() {
    // A request's argument fills a marker when composing; it never becomes a reference,
    // or a caller could make the template read what they name.
    // Angle brackets make no IRI, so the check page refuses the value, as the input
    // reader would, and reads nothing.
    let host = Host::new();
    match host.source_with(
        "urn:toolbox:check:urn:example:data",
        &[("shapes", "<urn:example:secret>")],
        &no_scopes(),
    ) {
        Err(Error::InvalidArgument { name, .. }) => assert_eq!(name, "shapes"),
        other => panic!("expected InvalidArgument on `shapes`, got {other:?}"),
    }
    let refused = Host::new().source_with(
        "urn:toolbox:page",
        &[("base", "<urn:example:secret>")],
        &no_scopes(),
    );
    match refused {
        Err(Error::InvalidArgument { name, .. }) => assert_eq!(name, "base"),
        other => panic!("expected InvalidArgument on `base`, got {other:?}"),
    }
}

#[test]
fn a_reference_to_shapes_is_a_name_and_is_not_read() {
    // `shapes` is declared xsd:anyURI: the kernel hands the view the IRI's text.
    let request = Request::new(
        Verb::Source,
        Iri::parse("urn:toolbox:check:urn:example:data").unwrap(),
    )
    .with_arg(
        "shapes",
        ArgRef::Reference(Iri::parse("urn:example:secret").unwrap()),
    );
    let page = block_on(Host::new().kernel.issue(request, &no_scopes()))
        .expect("naming a resource needs no authority over it");
    let html = String::from_utf8(page.bytes).unwrap();
    assert!(
        html.contains("/toolbox/input/urn%3Aexample%3Asecret"),
        "{html}"
    );
    assert!(!html.contains(DATA), "{html}");
}

#[test]
fn the_check_page_reads_under_the_callers_capability() {
    // The page names the resource and reads none of it; its input is what reads, and that
    // read is the caller's. The grant goes first on one kernel, so a denied caller after it
    // shows the cached answer is not handed across.
    let host = Host::new();
    let check = "urn:toolbox:check:urn:example:secret";
    for capability in [granted(), no_scopes()] {
        let page = host
            .source_with(check, &[], &capability)
            .expect("the page itself holds nothing guarded");
        let html = String::from_utf8(page.bytes).unwrap();
        assert!(!html.contains(DATA), "{html}");
        assert_eq!(
            input_names(&html),
            ["urn:toolbox:input:urn:example:secret"],
            "{html}"
        );
    }
    let input = "urn:toolbox:input:urn:example:secret";
    let read = host
        .source_with(input, &[], &granted())
        .expect("a caller who can read it sees it");
    assert_eq!(read.bytes, DATA.as_bytes());
    match host.source_with(input, &[], &no_scopes()) {
        Err(Error::Denied(_)) => {}
        other => panic!("a caller who cannot read it is refused, got {other:?}"),
    }
}
