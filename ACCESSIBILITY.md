# Accessibility

## Scope and status

ikigai-toolbox serves the RDF Toolbox through `urn:toolbox:*`: the static
`toolbox.html` page and the composed `urn:toolbox:page` and
`urn:toolbox:check:{iri}` views. The page opens, pastes and edits RDF/SPARQL
files, validates and formats them, checks SHACL shapes, and shows SPARQL
syntax diagnostics. Its WebAssembly engine runs in a browser worker.
The host supplies the HTTP routes and surrounding UI.

This is a limited source review, dated **2026-10-31**, of turtle-lsp **v0.5.2**,
commit `dcfeed02885c012ad5b7d1c28f33151eda8d75f8`, plus this repository's
patches and token stylesheet. **WCAG 2.2 AA conformance has not been
established.** The features below are implementation evidence, not a claim
that entire success criteria pass.

## Accessibility features present

- The page declares English and a title, has a main landmark and named Input
  and Tool sections, and gives the text editor, paste field and sample selector
  accessible names. The highlighted copy of the editor is `aria-hidden`.
- Tool navigation uses tab/tablist/tabpanel roles, selected state, panel
  associations and a roving tab stop. Left/Right arrow handlers select and
  focus adjacent tabs. File removal buttons include the filename in their name.
- Open files, Open folder and Paste provide alternatives to drag and drop.
  Patch `0003-input-names.patch` adds accessible names to the hidden file
  inputs; the composed template retains them.
- Engine failures and other notifications use a `role="alert"` container.
  This does not cover ordinary analysis results (see below).
- The composed views load `web/tokens.css` after upstream CSS, supporting
  light, dark and system-selected themes. It maps faint text to the shared
  muted color and defines a focus-visible outline. The static `toolbox.html`
  loads only upstream CSS, so these local improvements do not apply there.

## Known gaps and unverified behavior

- Routine analysis results, counts and busy state are updated without a live
  region or `aria-busy`; automatic result announcements are not implemented.
  Notifications disappear after six seconds without a persistent history.
- Rendering replaces tab, file-list and result markup. Focus retention during
  asynchronous analysis and file removal has not been verified in a browser.
- Upstream's editor textarea suppresses its outline. Its more specific selector
  overrides the local generic focus-visible rule, so that rule alone does not
  establish a visible editor focus indicator.
- The local muted-text adjustment does not establish contrast for syntax colors,
  diagnostic colors, controls or all backgrounds. The static page retains
  upstream's faint-text palette. There is no dedicated high-contrast palette;
  forced-colors behavior has not been tested.
- Keyboard operation end to end, screen-reader reading and announcements,
  zoom/reflow, text spacing, target sizes and mobile interaction have not been
  tested. The transparent textarea/highlight overlay particularly needs checks
  with forced colors, zoom and assistive technology.

## What was checked

The review inspected the pinned upstream `web/app/toolbox.html`, `app.js` and
`style.css`, local patches `0001`–`0004`, `web/tokens.css`, the committed
`tests/fixtures/page.template.html`, and existing test assertions. It confirmed
page serving in `src/lib.rs` and the absence of this document before the change.
No browser, screen-reader or automated accessibility audit was performed.

CI's `tests/views.rs` checks composed views, including named file inputs;
`node tests/smoke.mjs` checks the built patches, template fixture and token
mapping alongside engine behavior. These are source/output regression checks,
not accessibility conformance tests. CI results for this documentation change
are recorded on its pull request.

Before claiming conformance, test both static and composed pages in light and
dark themes with keyboard-only operation and a screen reader, covering file
open/paste/edit/remove, every tool, asynchronous results and failures. Check
focus, contrast, zoom/reflow and forced colors on the actual host deployment.

## Ownership and reporting

The page, JavaScript, base CSS and engine come from
[turtle-lsp](https://github.com/abhishekkundu25/turtle-lsp), by Abhishek Kundu,
under the MIT license (Copyright 2026 Abhishek Kundu). Each build preserves that
notice as `LICENSE.turtle-lsp`. Upstream page fixes belong there and must also
be evaluated when updating this repository's pin.

This repository owns its integration, patches and `web/tokens.css`, licensed
MIT OR Apache-2.0. Hosts own their routing and surrounding UI. Report problems
with this integration in [ikigai-toolbox issues](https://github.com/ikigai-rs/ikigai-toolbox/issues),
including the page variant, build/pin, browser, assistive technology and steps
to reproduce; identify upstream page behavior when known. See the
[README](README.md#the-patches-we-carry) for local patches and build provenance.
