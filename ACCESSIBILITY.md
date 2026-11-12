# Accessibility

What `ikigai-toolbox` shows people, the rules its own part of that output follows, how it is
tested, and how to report a barrier. It is not a conformance claim. Adapted from the
[ACCESSIBILITY.md template](https://github.com/mgifford/ACCESSIBILITY.md) (MIT).

| Field | Value |
|---|---|
| Target | WCAG 2.2 Level AA, for the pages this repository serves |
| Status | Evaluation in progress. No conformance claim. |
| Owner | the maintainers of this repository |
| Report a barrier | [a GitHub issue](https://github.com/ikigai-rs/ikigai-toolbox/issues/new) with "Accessibility" in its title |
| Last reviewed | 2026-11-12 |

## What it shows people

- **The RDF Toolbox page**, served through `urn:toolbox:*`: the static `toolbox.html` page and
  the composed `urn:toolbox:page` and `urn:toolbox:check:{iri}` views. The page opens, pastes
  and edits RDF/SPARQL files, validates and formats them, checks SHACL shapes, and shows
  SPARQL syntax diagnostics. Its WebAssembly engine runs in a browser worker. These are HTML,
  CSS and JavaScript.
- **Documentation**: this file, the README and the CHANGELOG, Markdown read on GitHub or in an
  editor.
- No terminal interface and no other generated HTML or picture.

The page, its JavaScript, base CSS and engine come from
[turtle-lsp](https://github.com/abhishekkundu25/turtle-lsp) **v0.5.2**, commit
`dcfeed02885c012ad5b7d1c28f33151eda8d75f8`, by Abhishek Kundu, under the MIT license
(Copyright 2026 Abhishek Kundu); each build preserves that notice as `LICENSE.turtle-lsp`.
This repository owns its integration, patches `0001`–`0004` and `web/tokens.css`, licensed
MIT OR Apache-2.0. Hosts own their HTTP routes and surrounding UI. See the
[README](README.md#the-patches-we-carry) for the patches and build provenance.

This repository does not use ikigai-ui's per-kind accessibility criteria
(`urn:iki:ui:criterion:kind:{kind}:target:{target}`): it composes no ikigai-ui kind, so none
of their SEE or HEAR checks apply to its tests.

## What the page provides

Found by reading the pinned source, the patches and `web/tokens.css` (first on 2026-10-31,
rechecked against main on 2026-11-12). These are implementation features, not a claim that
any success criterion passes.

- The page declares English and a title, has a main landmark and named Input and Tool
  sections, and gives the text editor, paste field and sample selector accessible names. The
  highlighted copy of the editor is `aria-hidden`.
- Tool navigation uses tab/tablist/tabpanel roles, selected state, panel associations and a
  roving tab stop. Left/Right arrow handlers select and focus adjacent tabs. File removal
  buttons include the filename in their name.
- Open files, Open folder and Paste provide alternatives to drag and drop. Patch
  `0003-input-names.patch` names the hidden file inputs; the composed template keeps them.
- Engine failures and other notifications use a `role="alert"` container.
- The composed views load `web/tokens.css` after upstream CSS, for light, dark and
  system-selected themes. It maps upstream's faint text to the shared muted color and defines
  a focus-visible outline. The static `toolbox.html` loads only upstream CSS, so these local
  changes do not apply there.

## How it is tested

Every named check has a result: Pass, Fail or Blocked, with evidence. A Pass covers only the
stated check, not overall accessibility. All checks are on main at commit `737683f`.

| Check | Result | Evidence |
|---|---|---|
| Composed view markup: language, theme attribute, named file inputs, escaping, snapshots | Pass | `cargo test --test views` (`tests/views.rs`, 13 tests), rustc 1.99.1, 2026-11-12; CI's `rust.yml` job repeats it on every push and pull request. Checks the composed HTML as text; renders nothing. |
| Built page: patches applied (named file inputs), template fixture, faint-to-muted token mapping | Pass | `node tests/smoke.mjs` after `./build.sh`, Node v22.23.4, in CI's `dist` job on main (run [43047073474](https://github.com/ikigai-rs/ikigai-toolbox/actions/runs/43047073474), 2026-11-11), repeated on every push and pull request. Checks build output as text; renders nothing. |
| Local token text and focus-ring contrast | Pass | WCAG relative-luminance ratios of `web/tokens.css` values, computed with a Python 3.15.2 script, 2026-11-12: muted text 6.17:1 (light, on surface) and 7.17:1 (dark, on surface); focus ring at least 6.16:1 on page and surface in both themes. CI does not repeat it. Covers the shared tokens only, not upstream's syntax, diagnostic or control colors. |
| Status messages for analysis results (SC 4.1.3) | Fail | Source reading of the pinned `toolbox.html` and `app.js`, 2026-11-12: results, counts and busy state change with no `aria-live`, `role="status"` or `aria-busy`. Only notifications, such as an engine crash, reach the `role="alert"` container, and they disappear after six seconds. |
| Upstream syntax, diagnostic and control colors; static page's faint text | Blocked | Not measured. The static page keeps upstream's `--faint`, which the `web/tokens.css` comment records as 2.35:1 (light) and 3.73:1 (dark) on its surface. |
| Editor focus indicator | Blocked | Upstream's `.editor textarea { outline: none }` outweighs the local `:focus-visible` rule, so that rule does not draw a ring on the editor; whether the caret alone is a visible indicator has not been checked in a browser. |
| Browser rendering: focus retention after re-render, zoom and reflow, text spacing, target size, forced colors, mobile | Blocked | No browser run. Rendering replaces tab, file-list and result markup; the transparent textarea over its highlighted copy particularly needs forced-color and zoom checks. |
| Manual keyboard | Blocked | No person has performed a keyboard pass; no interactive browser session was available for this update. |
| Automated accessibility checker (axe) | Blocked | No axe or other automated accessibility checker has been run on this output; none is configured in CI. |
| Screen reader | Blocked | No screen reader has been used on this output; no screen-reader session was available for this update. |
| Screen magnifier | Blocked | No screen magnifier has been used on this output; no magnifier session was available for this update. |
| Speech input | Blocked | No speech-input software has been used on this output; no speech-input session was available for this update. |
| Switch access | Blocked | No switch-access software has been used on this output; no switch-access session was available for this update. |
| Braille display | Blocked | No braille display has been used on this output; no braille display was available for this update. |

Before any conformance claim, test both the static and composed pages in light and dark
themes with keyboard-only operation and a screen reader, covering file open, paste, edit and
remove, every tool, asynchronous results and failures, and check focus, contrast, zoom,
reflow and forced colors on the actual host deployment.

## Known limitations

- Analysis results are not announced (the Fail above), and notifications have no persistent
  history.
- There is no high-contrast palette; `web/tokens.css` leaves a seam for the shared one.
- Upstream page fixes belong upstream and must be evaluated again whenever this repository's
  pin moves.

## Contributing

Read this file before changing the patches, `web/tokens.css`, the page template or the pin.
In the pull request's `## Accessibility` section, give each check a row in a
Check | Result | Evidence table: Pass, Fail or Blocked, with the test or command, tool and
version used, or why it could not run. Keep unrun checks as Blocked, including a screen
reader nobody ran; never call it "not applicable".

## Reporting a barrier

[Open an issue](https://github.com/ikigai-rs/ikigai-toolbox/issues/new) with
"Accessibility" in its title: the page variant (static or composed), the build or pin, your browser
and any assistive technology if you are comfortable sharing them, and the steps to
reproduce; say whether it is upstream page behavior, if you know. You never need to disclose
a disability.
