# ikigai-toolbox

The **RDF Toolbox**, served as [ikigai](https://github.com/ikigai-rs) resources: Validate, Format,
SHACL and SPARQL over a host's own Turtle, running in the viewer's browser tab.

```text
/toolbox/check/urn%3Alayer%3Atalk%3Aentry%3A7?shapes=urn%3Alayer%3Atalk%3Ashapes#shacl
```

That URL opens the toolbox on one resource and checks it against a layer's closed shapes. The
resource is read through the host's kernel, under the viewer's own authority, and the SHACL engine
runs in the page. Nothing is uploaded, and nothing is pasted.

## Credit

The toolbox is **[turtle-lsp](https://github.com/abhishekkundu25/turtle-lsp)** by **Abhishek
Kundu**, a Rust RDF language server that holds itself to the W3C suites: RDF syntaxes 1137/1137,
SPARQL syntax 425/426, SHACL Core 98/98, SHACL-SPARQL 23/23. Its
[RDF Toolbox page](https://abhishekkundu25.github.io/turtle-lsp/toolbox.html) runs the server as
WebAssembly in a worker. This repository vendors that page and serves it. All of the checking is
upstream's work.

turtle-lsp is MIT-licensed, Copyright (c) 2026 Abhishek Kundu. Every build carries upstream's
copyright and permission notice as `LICENSE.turtle-lsp`, served at `urn:toolbox:LICENSE.turtle-lsp`.
This crate, the build script, the patches and `web/tokens.css` are `MIT OR Apache-2.0`.

## What a host serves

| name | what it is | how |
|---|---|---|
| `urn:toolbox:{name}` | a built file, from a fixed allowlist (`FILES`) | the host's build directory, relabeled with the type the name fixes |
| `urn:toolbox:manifest` | what the build holds and where it came from (JSON-LD) | alias of `urn:toolbox:MANIFEST.jsonld` |
| `urn:toolbox:template:page` | the page as an ikigai-fn compose template | alias of `urn:toolbox:page.template.html` |
| `urn:toolbox:page` | the empty toolbox (`theme=`) | `composeOver` the template |
| `urn:toolbox:check:{iri}` | the toolbox checking one resource (`shapes=`, `theme=`) | `composeOver` the template |
| `urn:toolbox:input:{iri}` | a resource's Turtle face, as text for the page | the resource read with `as=text/turtle` under the caller's capability, transrepted if it answers another RDF syntax |
| `urn:toolbox:text:{text}`, `urn:toolbox:none` | the template's helpers: a value as text, and nothing | ikigai-fn's `echo`, and one constant |

The allowlist is the page (`toolbox.html`, `app.js`, `samples.js`, `worker.js`, `engine.js`,
`style.css`), the engine (`engine.wasm`), `tokens.css`, `page.template.html`, `LICENSE.turtle-lsp`
and `MANIFEST.jsonld`. Any other name under `urn:toolbox:` is `NotFound`, and the base is never
asked, so the family is not a general file server.

Every name is the same for every caller, so the family requires no capability of its own. Whatever
it reads, the build directory and every input, it reads under the **caller's** capability, so a
viewer sees only what they could read anyway. A host that binds its build with ikigai-fs grants its
`/toolbox` door `urn:cap:fs:read:<the build directory>`.

### Mounting it

```rust
use std::sync::Arc;
use ikigai_core::{EndpointSpace, Fallback, Kernel, UriTemplate};

let build = EndpointSpace::new().bind(
    UriTemplate::parse("urn:iki:toolbox:dist:{path}").unwrap(), // ikigai_toolbox::DIST
    ikigai_fs::FileEndpoint::new("/srv/toolbox/dist"),
);
let kernel = Kernel::new(Arc::new(Fallback::new(vec![Arc::new(ikigai_toolbox::space()), Arc::new(build)])));
```

`space()` names itself `urn:iki:space:toolbox` (`SPACE_ID`). It reads its build at the fixed name
`DIST`, so every call holds the same doors. `space_at(base)` reads another base and stays
anonymous, because only the host knows which instance it is.

Then route `/toolbox/{…}` to `urn:toolbox:{…}` by the mechanical rule (path segments joined by `:`,
each percent-decoded), with **`TOOLBOX_CSP`** on the route:

```text
default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; img-src 'self' data:;
connect-src 'self'; worker-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none';
form-action 'none'
```

The page has no inline script or style. `'wasm-unsafe-eval'` lets the worker compile the engine,
and that is all it allows. The CSP is door configuration: no artifact here sets a header.

### Caching

Each file's answer hangs from the base's golden thread. ikigai-fs reads are live by default, so the
toolbox's files are read fresh every time. An uncached `urn:toolbox:engine.wasm` (8.3 MB) took a
median 0.39 ms over ikigai-fs in a release build (macOS, warm page cache;
`cargo test --release --test host -- --ignored --nocapture`). `build.sh` writes outside the kernel,
so a host that mounts the build directory **cacheable** must also watch it, so that a rebuild cuts
the thread. Otherwise it serves the old build until something else does. Serving it uncached is the
simple choice, and at that cost it is the recommended one.

## The page over a host's own resources

- **`/toolbox/check/{iri}`** is `urn:toolbox:check:{iri}`: the template composed with the
  resource's IRI. The page's `<body>` carries RDFa `about="{iri}"`, and the header links to the
  resource's text face. `shapes=` adds a second input, so the SHACL tab checks the resource
  against those shapes. `theme=` (`auto`, `light`, `dark`) sets `data-theme` on `<html>`. Every
  value goes into the template with `$h`, which escapes it, and a value spliced into a path is
  percent-encoded as one segment.
- **`/toolbox/toolbox.html?from=/a.ttl&from=/a.shapes.ttl#shacl`** is the static page fetching
  same-origin paths. The fragment still picks the tool.

Either way the page reads its inputs with the patched `app.js`. An input must start with `/`, must
not start with `//`, and must resolve to the page's own origin. Anything else gets a visible note
and is never fetched. The host's `connect-src 'self'` is the second fence.

## Building `dist/`

```sh
git submodule update --init
./build.sh            # needs rustup, git and node
node tests/smoke.mjs  # the engine, in Node, over real fixtures
```

`build.sh` refuses unless `vendor/turtle-lsp` is at the pinned commit and clean. It then builds the
engine (`web/engine`, a cdylib with a plain C ABI and no wasm-bindgen) for
`wasm32-unknown-unknown` with upstream's own toolchain, **Rust 1.99.0** from
`vendor/turtle-lsp/rust-toolchain.toml`. The build goes into `target/engine-<commit>`, never into
the submodule. Next the script copies the page, applies the patches to the copies, and writes
`MANIFEST.jsonld`: each file's IRI, media type, size and `sha256-` digest, and the upstream commit
it came from. The wasm is always built here from the pinned source, never downloaded from
upstream's site.

**The pin: turtle-lsp `v0.5.2`, commit `dcfeed02885c012ad5b7d1c28f33151eda8d75f8`.** The commit
is what `build.sh` checks, because a tag can move.

### The patches we carry

Each patch is a diff against the pinned file, applied to the copy in `dist/`:

- **`0001-open-from.patch`** lets `app.js` open its input from the page: `?from=` (repeatable) and
  the body's `data-inputs` (a JSON array the template sets). It adds one function and its call.
  Upstream's page reads only files the viewer picks, drops or pastes.
- **`0002-editor-link.patch`** points "Open the editor" at upstream's hosted editor, because we do
  not vendor Monaco.
- **`0003-page-template.patch`** makes a copy of the patched `toolbox.html` into
  `page.template.html`, the compose template behind `urn:toolbox:page` and
  `urn:toolbox:check:{iri}`. The template takes the theme, the self-description and the inputs,
  and references its assets at absolute `/toolbox/…` paths, because a check page lives below
  `/toolbox/check/`.

`web/tokens.css` is ours. It maps upstream's custom properties (`--panel`, `--tx`, `--acc`,
`--on-acc`) onto the ikigai token names (`--surface`, `--fg`, `--accent`, `--accent-fg`) and is
loaded after upstream's stylesheet. Its palette values are gonk's for now. The shared token
stylesheet will replace them, and the mapping will stay.

### Moving the pin

1. `git -C vendor/turtle-lsp fetch --tags && git -C vendor/turtle-lsp checkout <new commit>`, then
   `git add vendor/turtle-lsp`.
2. Set `TAG` and `COMMIT` in `build.sh`, and the tag and commit in this README.
3. Run `./build.sh`. A patch that no longer applies stops it: refresh that patch against the new
   file (`git diff --no-index`), keeping it to the same change.
4. Copy `dist/page.template.html` and `dist/MANIFEST.jsonld` over `tests/fixtures/`, rerun
   `node tests/smoke.mjs` and `UPDATE_SNAPSHOTS=1 cargo test --test views`, and read the diffs.

## Not compiled in

The UI conventions deliver an artifact two ways: from a directory, or compiled into the binary
under the same IRIs. This crate does the **directory** only. The engine is 8.3 MB of WebAssembly,
which does not belong inside a published crate. A host that wants a standalone binary can embed the
built directory behind a feature of its own and bind it at `DIST`, and the names stay the same.

## Tests

- `tests/smoke.mjs` (CI's `dist` job): the engine checks a valid and a broken Turtle file, a real
  layer's closed SHACL shapes (ikigai-layers-client's `talk.shapes.ttl`) against a conforming
  message, one with an unknown property and one with a non-IRI `ik:inReplyTo`, and a SPARQL query
  with one error. It also checks the manifest against the files and checks that every patch applied.
- `tests/conformance.rs`: the module recipe (ikigai-conformance 0.10, FAILURES opted in).
- `tests/files.rs`: the allowlist and its media types, the refusals (`../x`, `%2e%2e/x`, an
  unlisted name: `NotFound` with no sub-request), NotFound passthrough, and the manifest's Turtle
  face (through ikigai-rdf's transreptor).
- `tests/views.rs`: snapshots of the page and a check page, a hostile `theme=` and a hostile IRI
  escaped, and an input refused to a caller who cannot read it.
- `tests/host.rs`: the mount above, with ikigai-fs over a real directory.

## License

`MIT OR Apache-2.0` for this repository. turtle-lsp, vendored as a submodule and copied into every
build, is MIT, Copyright (c) 2026 Abhishek Kundu (`LICENSE.turtle-lsp`).
