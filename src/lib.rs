//! `ikigai-toolbox`: the RDF Toolbox, [turtle-lsp] running in the browser, served as
//! ikigai resources.
//!
//! The toolbox is a page and a WebAssembly engine that check RDF in the viewer's own
//! tab: Validate, Format, SHACL and SPARQL over files the page is given. This crate
//! holds **no state and no file I/O**. A host builds the page once (`build.sh` in this
//! repository writes a directory, `dist/`), binds that directory at a base IRI, and
//! mounts [`space`]. Every name below is the same for every caller, so the family
//! requires no capability of its own: what a name reads, it reads under its CALLER's
//! authority.
//!
//! [turtle-lsp]: https://github.com/abhishekkundu25/turtle-lsp
//!
//! ## The names
//!
//! | name | what it is |
//! |---|---|
//! | `urn:toolbox:{name}` | a built file: [`FILES`] is the allowlist and fixes each one's media type |
//! | `urn:toolbox:manifest` | an alias of `urn:toolbox:MANIFEST.jsonld`: what the build holds and where it came from |
//! | `urn:toolbox:template:page` | an alias of `urn:toolbox:page.template.html`: the page as an ikigai-fn compose template |
//! | `urn:toolbox:page` | the empty toolbox: the template composed with no inputs |
//! | `urn:toolbox:check:{iri}` | the toolbox checking one resource, with an optional `shapes=` IRI |
//! | `urn:toolbox:input:{iri}` | a resource's Turtle face as text, read under the caller's capability |
//! | `urn:toolbox:path:{rest}` | the template's paths: `rest` under the `base=` the page is served from |
//! | `urn:toolbox:text:{text}`, `urn:toolbox:none` | the template's two helpers: a value as text, and nothing |
//!
//! A host routes `/toolbox/{…}` to these by the mechanical rule (path segments joined
//! by `:`), with [`TOOLBOX_CSP`] on the route: `/toolbox/toolbox.html`,
//! `/toolbox/check/{iri}?shapes={iri}`, `/toolbox/input/{iri}`. A host that serves the
//! family under another prefix passes it to the views as `base=` ([`DEFAULT_BASE`] is
//! `/toolbox/`), and the composed page names its assets and inputs under it.
//!
//! ## Code, and what is composed
//!
//! Two endpoints are code, each because composition cannot relabel a media type:
//! the file reader (the base answers `application/octet-stream`, and the type is a
//! fact about the NAME, not the bytes) and the input reader (a resource's Turtle face
//! served as text for the page to fetch). The views are ikigai-fn `composeOver` over
//! the template; the aliases are core's `AliasTable`; `urn:toolbox:text:{text}` is
//! ikigai-fn's `echo`. The one constant is `urn:toolbox:none`, the empty text a
//! template fallback needs, and `urn:toolbox:path:{rest}` joins a path to the base,
//! which a template marker cannot.

#![forbid(unsafe_code)]

use std::sync::Arc;

use async_trait::async_trait;
use ikigai_core::{
    space_iri, Alias, AliasTable, ArgRef, ArgSpec, AsyncFnEndpoint, Body, Description, Endpoint,
    EndpointSpace, Error, Exact, FnEndpoint, Invocation, InvokeFuture, Iri, ReprType,
    Representation, Request, UriTemplate, Verb,
};

/// The name [`space`] claims: `urn:iki:space:toolbox`.
pub const SPACE_ID: &str = "urn:iki:space:toolbox";

/// Where [`space`] reads the built files: `urn:iki:toolbox:dist:{name}`.
///
/// A host binds its build directory here (ikigai-fs's `FileEndpoint` under the
/// template `urn:iki:toolbox:dist:{path}`, or anything that answers Source with the
/// file's bytes under `as=application/octet-stream`). It is a fixed name, not an
/// argument, so `space()` holds the same doors on every call and its name is a true
/// claim; [`space_at`] reads another base and is named by its host.
pub const DIST: &str = "urn:iki:toolbox:dist:";

/// The page a host links to: `toolbox.html`, served as `urn:toolbox:toolbox.html`.
pub const PAGE: &str = "toolbox.html";

/// The Content-Security-Policy a host puts on its `/toolbox/{…}` routes.
///
/// It is door configuration, never an artifact's (UI conventions D6, D17): the page
/// has no inline script or style, its favicon is a `data:` URI, and the engine is
/// WebAssembly compiled in a worker, which is what `'wasm-unsafe-eval'` allows and
/// all it allows. `connect-src 'self'` is the second fence behind the page's own
/// same-origin rule for the inputs it fetches.
///
/// ```
/// assert_eq!(
///     ikigai_toolbox::TOOLBOX_CSP,
///     "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; \
///      img-src 'self' data:; connect-src 'self'; worker-src 'self'; object-src 'none'; \
///      base-uri 'none'; frame-ancestors 'none'; form-action 'none'"
/// );
/// ```
pub const TOOLBOX_CSP: &str = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; \
style-src 'self'; img-src 'self' data:; connect-src 'self'; worker-src 'self'; \
object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'";

/// The allowlist: every file `urn:toolbox:{name}` may answer, with the media type its
/// NAME fixes. Nothing else under `urn:toolbox:` reads a file, so the family is never a
/// general file server.
///
/// The same table is `tools/manifest.mjs`'s, and `tests/media_types.rs` holds the two
/// equal.
///
/// ```
/// assert_eq!(ikigai_toolbox::media_type("engine.wasm"), Some("application/wasm"));
/// assert_eq!(ikigai_toolbox::media_type("app.js"), Some("text/javascript"));
/// assert_eq!(ikigai_toolbox::media_type("../etc/passwd"), None);
/// ```
pub const FILES: &[(&str, &str)] = &[
    ("toolbox.html", HTML),
    ("page.template.html", HTML),
    ("app.js", JS),
    ("samples.js", JS),
    ("worker.js", JS),
    ("engine.js", JS),
    ("style.css", CSS),
    ("tokens.css", CSS),
    ("engine.wasm", "application/wasm"),
    ("LICENSE.turtle-lsp", TEXT),
    ("MANIFEST.jsonld", "application/ld+json"),
];

const HTML: &str = "text/html; charset=utf-8";
const JS: &str = "text/javascript";
const CSS: &str = "text/css";
const TEXT: &str = "text/plain; charset=utf-8";
const OCTET_STREAM: &str = "application/octet-stream";
const TURTLE: &str = "text/turtle";
/// The datatype of an input whose value is the IRI of another resource (ikigai-fn's
/// convention for compose's `src`).
const XSD_ANY_URI: &str = "http://www.w3.org/2001/XMLSchema#anyURI";
const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";

/// The media type `name` is served with, or `None` if `name` is not in [`FILES`].
pub fn media_type(name: &str) -> Option<&'static str> {
    FILES.iter().find(|(n, _)| *n == name).map(|(_, t)| *t)
}

/// The template the views compose: `urn:toolbox:template:page`.
pub const TEMPLATE: &str = "urn:toolbox:template:page";

/// The path a host serves the family under, when a view is given no `base=`:
/// `/toolbox/`, the prefix of the mechanical route.
pub const DEFAULT_BASE: &str = "/toolbox/";

/// Whether `base` can prefix the page's paths: path-absolute (it starts with `/` and
/// not `//`), ends with `/`, and holds only the characters of a URI path (RFC 3986
/// `pchar` and `/`), so no backslash, space, quote, query or fragment. Anything that
/// passes stays a same-origin path whatever is appended to it.
///
/// ```
/// assert!(ikigai_toolbox::is_base("/toolbox/"));
/// assert!(ikigai_toolbox::is_base("/ikigai-tutorial/toolbox/"));
/// assert!(!ikigai_toolbox::is_base("//evil.example/"));
/// assert!(!ikigai_toolbox::is_base("/\\evil.example/"));
/// assert!(!ikigai_toolbox::is_base("javascript:alert(1)//"));
/// assert!(!ikigai_toolbox::is_base("/toolbox"));
/// ```
pub fn is_base(base: &str) -> bool {
    let pchar = |c: char| c.is_ascii_alphanumeric() || "-._~!$&'()*+,;=:@%/".contains(c);
    base.starts_with('/')
        && !base.starts_with("//")
        && base.ends_with('/')
        && base.chars().all(pchar)
}

fn base_spec() -> ArgSpec {
    ArgSpec::new("base")
        .summary(
            "the path the toolbox is served under, path-absolute and ending in `/`: the \
             page's assets and inputs are named below it",
        )
        .class(XSD_STRING)
        .default_value(DEFAULT_BASE)
}

/// The request's `base=`, refused unless it [`is_base`]; [`DEFAULT_BASE`] when absent
/// (the kernel injects the declared default, so absent is only a direct invocation).
fn base_of<'a>(inv: &'a Invocation<'_>) -> Result<&'a str, Error> {
    let base = match inv.inline_str("base") {
        Ok(base) => base,
        Err(Error::MissingArgument(_)) => DEFAULT_BASE,
        Err(other) => return Err(other),
    };
    if !is_base(base) {
        return Err(Error::InvalidArgument {
            name: "base".to_string(),
            detail: format!(
                "`{base}` is not a path-absolute base ending in `/` (like `{DEFAULT_BASE}`)"
            ),
        });
    }
    Ok(base)
}

/// The palettes `theme=` names (UI conventions D12): `auto` follows the viewer's
/// system. The template writes the value into `data-theme` on `<html>`, escaped.
pub const THEMES: &[&str] = &["auto", "light", "dark"];

// --- the file reader -------------------------------------------------------------

/// `urn:toolbox:{name}`: Source `{base}{name}` and serve its answer with the type the
/// name fixes. A name outside [`FILES`] is `NotFound` with no sub-request; the base's
/// `NotFound` is `NotFound` for the name asked; every other failure passes through as
/// it came. Expiry and golden threads are the sub-request's.
///
/// Two paths, one answer. The body path (`invoke_body`, what the kernel calls) hands
/// the base's body through as it came (ledger #2290): an fs `Replay` keeps its
/// validator, its length and its range reads, and nothing is read on the way. The
/// bytes path (`invoke`) is the base's answer collected under the run's byte budget,
/// as before.
struct FileEndpoint {
    base: Arc<str>,
    description: Description,
}

impl FileEndpoint {
    /// The media type `urn:toolbox:{name}` fixes and the request for its file, or
    /// `NotFound` for a name outside [`FILES`].
    fn plan(&self, inv: &Invocation<'_>) -> Result<(&'static str, Request), Error> {
        let asked = inv.request.target.as_str();
        let name = inv.bindings.get("name").unwrap_or_default();
        let Some(media) = media_type(name) else {
            return Err(Error::NotFound(format!(
                "{asked}: not a toolbox file (the names are: {})",
                FILES.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")
            )));
        };
        let base = &self.base;
        let source = Iri::parse(format!("{base}{name}")).map_err(|e| {
            Error::Endpoint(format!("{asked}: the base `{base}` makes no IRI: {e}"))
        })?;
        let request = Request::new(Verb::Source, source)
            .with_arg("as", ArgRef::Inline(OCTET_STREAM.as_bytes().to_vec()));
        Ok((media, request))
    }
}

/// The base's failure as the name's: its `NotFound` names the file asked.
fn as_asked(inv: &Invocation<'_>, error: Error) -> Error {
    match error {
        Error::NotFound(why) => Error::NotFound(format!("{}: {why}", inv.request.target.as_str())),
        other => other,
    }
}

#[async_trait]
impl Endpoint for FileEndpoint {
    async fn invoke(&self, inv: &Invocation<'_>) -> Result<Representation, Error> {
        let (media, request) = self.plan(inv)?;
        let rep = inv.issue(request).await.map_err(|e| as_asked(inv, e))?;
        Ok(Representation::new(ReprType::new(media), rep.bytes).cacheable())
    }

    async fn invoke_body(&self, inv: &Invocation<'_>) -> Result<Representation<Body>, Error> {
        let (media, request) = self.plan(inv)?;
        let rep = inv
            .issue_body(request)
            .await
            .map_err(|e| as_asked(inv, e))?;
        Ok(Representation::from_body(ReprType::new(media), rep.bytes).cacheable())
    }

    fn name(&self) -> &str {
        "toolbox-file"
    }

    fn describe(&self) -> Description {
        self.description.clone()
    }
}

fn file_endpoint(base: Arc<str>) -> FileEndpoint {
    FileEndpoint {
        base,
        description: Description::new("toolbox-file")
            .title("A toolbox file")
            .summary(
                "One file of the RDF Toolbox build (turtle-lsp in the browser), by name from a \
                 fixed allowlist, served with the media type the name fixes. Read from the host's \
                 build directory under the caller's capability; any other name is not found.",
            )
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .input(
                ArgSpec::new("name")
                    .summary("the file's name in the build")
                    .class(XSD_STRING)
                    .binding()
                    .one_of(FILES.iter().map(|(n, _)| *n)),
            )
            .output(HTML)
            .output(JS)
            .output(CSS)
            .output("application/wasm")
            .output(TEXT)
            .output("application/ld+json"),
    }
}

// --- the input reader ------------------------------------------------------------

/// `urn:toolbox:input:{iri}`: the resource's Turtle face, as `text/plain` for the page
/// to read. Sourced with `as=text/turtle` under the CALLER's capability (so a viewer
/// sees only what they could read directly); an answer in another RDF syntax goes
/// through the kernel's transreptor plan to Turtle. Declares no capability of its own.
fn input_endpoint() -> AsyncFnEndpoint {
    AsyncFnEndpoint::new(
        "toolbox-input",
        |inv: &Invocation<'_>| -> InvokeFuture<'_> {
            Box::pin(async move {
                let raw = inv
                    .bindings
                    .get("iri")
                    .ok_or_else(|| Error::MissingArgument("iri".to_string()))?;
                let target = Iri::parse(raw).map_err(|e| Error::InvalidArgument {
                    name: "iri".to_string(),
                    detail: format!("not an IRI: {e}"),
                })?;
                let request = Request::new(Verb::Source, target.clone())
                    .with_arg("as", ArgRef::Inline(TURTLE.as_bytes().to_vec()));
                let mut rep = inv.issue(request).await?;
                let essence = |t: &ReprType| {
                    t.media_type
                        .split(';')
                        .next()
                        .unwrap_or_default()
                        .trim()
                        .to_ascii_lowercase()
                };
                if essence(&rep.repr_type) != TURTLE {
                    let from = essence(&rep.repr_type);
                    let plan = inv.select_transreptor(&from, TURTLE).ok_or_else(|| {
                        Error::InvalidArgument {
                            name: "iri".to_string(),
                            detail: format!(
                                "`{}` answered `{from}`, and no transreptor reaches {TURTLE}",
                                target.as_str()
                            ),
                        }
                    })?;
                    for step in plan {
                        let via = Iri::parse(&step.endpoint).map_err(|e| {
                            Error::Endpoint(format!("bad transreptor IRI `{}`: {e}", step.endpoint))
                        })?;
                        let request = Request::new(Verb::Source, via)
                            .with_arg("content", ArgRef::Inline(rep.bytes))
                            .with_arg("as", ArgRef::Inline(step.to.into_bytes()));
                        rep = inv.issue(request).await?;
                    }
                }
                Ok(Representation::new(ReprType::new(TEXT), rep.bytes).cacheable())
            })
        },
    )
    .with_description(
        Description::new("toolbox-input")
            .title("A toolbox input")
            .summary(
                "The named resource's Turtle face as text, for the toolbox page to check. Read \
                 under the caller's capability: a viewer sees only what they could read anyway.",
            )
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .input(
                ArgSpec::new("iri")
                    .summary("the resource to read, as its Turtle face")
                    .class(XSD_ANY_URI)
                    .binding(),
            )
            .output(TEXT),
    )
}

// --- the views -------------------------------------------------------------------

/// A `composeOver` of the page template under its own name and contract.
///
/// ikigai-fn's `ComposeOver` describes itself with no inputs (its arguments are
/// compose's open set), so the view states its own ArgSpecs here and delegates every
/// answer to the composer unchanged. No view code: what the page says is the template.
///
/// The one thing a view checks itself is `base=`, before composing: it prefixes URLs
/// in the page, so a value that is not a same-origin path is refused, not spliced.
fn view(description: Description) -> AsyncFnEndpoint {
    let composer = Arc::new(
        ikigai_fn::compose_over(Iri::parse(TEMPLATE).expect("TEMPLATE is an IRI"))
            .named(description.id.clone()),
    );
    AsyncFnEndpoint::new(
        description.id.clone(),
        move |inv: &Invocation<'_>| -> InvokeFuture<'_> {
            let composer = composer.clone();
            Box::pin(async move {
                base_of(inv)?;
                composer.invoke(inv).await
            })
        },
    )
    .with_description(description.input(base_spec()))
}

fn theme_spec() -> ArgSpec {
    ArgSpec::new("theme")
        .summary("the palette: auto follows the viewer's system (UI conventions D12)")
        .class(XSD_STRING)
        .optional()
        .one_of(THEMES.iter().copied())
}

fn page_view() -> AsyncFnEndpoint {
    view(
        Description::new("toolbox-page")
            .title("The RDF Toolbox")
            .summary(
                "The toolbox page with no input: Validate, Format, SHACL and SPARQL over files \
                 the viewer opens, pastes or drops. Nothing is uploaded.",
            )
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .input(theme_spec())
            .output(HTML),
    )
}

fn check_view() -> AsyncFnEndpoint {
    view(
        Description::new("toolbox-check")
            .title("Check a resource in the RDF Toolbox")
            .summary(
                "The toolbox page checking one resource: its Turtle face is the input, and \
                 `shapes=` adds a shapes graph so the SHACL tab checks the resource against it.",
            )
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .input(
                ArgSpec::new("iri")
                    .summary("the resource to check")
                    .class(XSD_ANY_URI)
                    .binding(),
            )
            .input(
                ArgSpec::new("shapes")
                    .summary("a resource holding SHACL shapes to check it against")
                    .class(XSD_ANY_URI)
                    .optional(),
            )
            .input(theme_spec())
            .output(HTML),
    )
}

/// `urn:toolbox:path:{rest}`: `{base}{rest}` as text, the template's way to name a
/// path under the `base=` it was composed with. `rest` is spliced as the marker wrote
/// it, so an argument in it stays the one percent-encoded segment compose made it.
fn path_endpoint() -> FnEndpoint {
    FnEndpoint::new("toolbox-path", |inv: &Invocation<'_>| {
        let rest = inv
            .bindings
            .get("rest")
            .ok_or_else(|| Error::MissingArgument("rest".to_string()))?;
        let path = format!("{}{rest}", base_of(inv)?);
        Ok(Representation::new(ReprType::new(TEXT), path.into_bytes()).cacheable())
    })
    .with_description(
        Description::new("toolbox-path")
            .title("A toolbox path")
            .summary(
                "A path under the base the toolbox is served from, as text: what the page \
                 template names its assets and inputs with.",
            )
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .input(
                ArgSpec::new("rest")
                    .summary("the path below the base")
                    .class(XSD_STRING)
                    .binding(),
            )
            .input(base_spec())
            .output(TEXT),
    )
}

/// `urn:toolbox:none`: the empty text. A template fallback names a resource, never a
/// literal, so "no value" has to be one.
fn none_endpoint() -> FnEndpoint {
    FnEndpoint::new("toolbox-none", |_: &Invocation<'_>| {
        Ok(Representation::new(ReprType::new(TEXT), Vec::new()).cacheable())
    })
    .with_description(
        Description::new("toolbox-none")
            .title("Nothing")
            .summary("The empty text: what the page template splices where a value is absent.")
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .output(TEXT),
    )
}

// --- the space -------------------------------------------------------------------

fn template(pattern: &str) -> UriTemplate {
    UriTemplate::parse(pattern).expect("a valid URI template")
}

/// The doors, over `base`. The specific doors come first: `urn:toolbox:{name}` would
/// match every other name here, and the first matching door answers.
fn doors(base: &str) -> Alias {
    let doors = EndpointSpace::new()
        .bind(Exact::new("urn:toolbox:page"), page_view())
        .bind(Exact::new("urn:toolbox:none"), none_endpoint())
        .bind(template("urn:toolbox:text:{message}"), ikigai_fn::echo())
        .bind(template("urn:toolbox:path:{rest}"), path_endpoint())
        .bind(template("urn:toolbox:input:{iri}"), input_endpoint())
        .bind(template("urn:toolbox:check:{iri}"), check_view())
        .bind(
            template("urn:toolbox:{name}"),
            file_endpoint(Arc::from(base)),
        );
    let aliases = AliasTable::new()
        .exact("urn:toolbox:manifest", "urn:toolbox:MANIFEST.jsonld")
        .exact(TEMPLATE, "urn:toolbox:page.template.html");
    Alias::new(Arc::new(aliases), Arc::new(doors))
}

/// The toolbox, reading its build at [`DIST`], named [`SPACE_ID`].
///
/// Configuration-free: it takes no parameters and reads nothing while it is built, so
/// every call holds the same doors and the name is a true claim. The host's half is
/// binding its build directory at [`DIST`]:
///
/// ```
/// # use std::sync::Arc;
/// # use futures::executor::block_on;
/// use ikigai_core::{
///     Capability, EndpointSpace, Fallback, FnEndpoint, Invocation, Iri, Kernel, ReprType,
///     Representation, Request, UriTemplate, Verb,
/// };
///
/// // Stands in for ikigai-fs's FileEndpoint over the build directory.
/// let dist = FnEndpoint::new("dist", |inv: &Invocation<'_>| {
///     let path = inv.bindings.get("path").unwrap_or_default();
///     Ok(Representation::new(ReprType::new("application/octet-stream"), path.as_bytes().to_vec()))
/// });
/// let host = EndpointSpace::new()
///     .bind(UriTemplate::parse("urn:iki:toolbox:dist:{path}").unwrap(), dist);
/// let kernel = Kernel::new(Arc::new(Fallback::new(vec![
///     Arc::new(ikigai_toolbox::space()),
///     Arc::new(host),
/// ])));
/// let css = block_on(kernel.issue(
///     Request::new(Verb::Source, Iri::parse("urn:toolbox:style.css").unwrap()),
///     &Capability::root(),
/// ))
/// .unwrap();
/// assert_eq!(css.repr_type.media_type, "text/css");
/// assert_eq!(css.bytes, b"style.css");
/// ```
pub fn space() -> Alias {
    // LAST: a name is a claim about the doors as built.
    doors(DIST).named(space_iri("toolbox"))
}

/// The toolbox reading its build at another `base` (`{base}{name}` for each file).
///
/// Anonymous: its doors read whatever base it was handed, so only the host knows
/// which instance it is, and the host names it (ledger #987's rule).
pub fn space_at(base: &str) -> Alias {
    doors(base)
}
