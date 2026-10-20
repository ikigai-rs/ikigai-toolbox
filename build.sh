#!/usr/bin/env bash
# Builds dist/ — the RDF Toolbox as this repo serves it — from the pinned
# upstream source in vendor/turtle-lsp, reproducibly:
#
#   1. refuse unless the submodule is at the pinned commit and clean;
#   2. build the engine (web/engine, a cdylib with a plain C ABI) for
#      wasm32-unknown-unknown, into a target dir of OURS keyed by the commit;
#   3. copy the page files, the wasm and upstream's LICENSE into dist/;
#   4. apply patches/*.patch in order to the copies (never to the submodule);
#   5. write dist/MANIFEST.jsonld (tools/manifest.mjs).
#
# Needs: git, rustup (upstream's rust-toolchain.toml picks the compiler, and
# the wasm32-unknown-unknown target for it), node.
set -euo pipefail

# The pin. The COMMIT is what is checked: a tag can move, a commit cannot.
UPSTREAM="https://github.com/abhishekkundu25/turtle-lsp"
TAG="v0.5.2"
COMMIT="dcfeed02885c012ad5b7d1c28f33151eda8d75f8"

ROOT="$(cd "$(dirname "$0")" && pwd)"
SUB="$ROOT/vendor/turtle-lsp"
DIST="$ROOT/dist"
# The page files, as upstream's web/app/ holds them: toolbox.html loads
# style.css and app.js; app.js imports samples.js and starts worker.js, which
# imports engine.js and fetches engine.wasm beside itself.
PAGE_FILES=(toolbox.html app.js samples.js style.css worker.js engine.js)

die() { printf 'build.sh: %s\n' "$*" >&2; exit 1; }

# 1. The submodule is the pinned commit, and clean.
[ -e "$SUB/.git" ] || die "vendor/turtle-lsp is not checked out (git submodule update --init)"
head="$(git -C "$SUB" rev-parse HEAD)"
[ "$head" = "$COMMIT" ] || die "vendor/turtle-lsp is at $head, not the pinned $COMMIT ($TAG)"
[ -z "$(git -C "$SUB" status --porcelain --untracked-files=all)" ] \
  || die "vendor/turtle-lsp has local changes; this build reads the pinned tree only"

# 2. The engine. CARGO_TARGET_DIR is ours and keyed by the commit, so no other
# revision's artifacts are ever linked (field guide 9h), and nothing is
# written into the submodule.
export CARGO_TARGET_DIR="$ROOT/target/engine-${COMMIT:0:12}"
started=$SECONDS
(cd "$SUB/web/engine" && cargo build --release --locked --target wasm32-unknown-unknown)
printf 'build.sh: engine built in %ss (toolchain %s)\n' "$((SECONDS - started))" \
  "$(cd "$SUB/web/engine" && rustc --version)"
WASM="$CARGO_TARGET_DIR/wasm32-unknown-unknown/release/turtle_lsp_web.wasm"
[ -f "$WASM" ] || die "the engine build produced no $WASM"
[ -z "$(git -C "$SUB" status --porcelain --untracked-files=all)" ] \
  || die "building the engine wrote into vendor/turtle-lsp"

# 3. The copies.
rm -rf "$DIST"
mkdir -p "$DIST"
for f in "${PAGE_FILES[@]}"; do cp "$SUB/web/app/$f" "$DIST/$f"; done
cp "$WASM" "$DIST/engine.wasm"
cp "$SUB/LICENSE" "$DIST/LICENSE.turtle-lsp"
# Ours, not upstream's: the token mapping (D12), loaded after style.css.
cp "$ROOT/web/tokens.css" "$DIST/tokens.css"

# 4. The patches, in order, onto the copies. The page template is made from the
# patched toolbox.html, so it carries 0001/0002 before 0003 makes it a template.
apply() { git -C "$ROOT" apply --directory=dist --whitespace=error "$1" || die "$(basename "$1") does not apply at $TAG"; }
apply "$ROOT/patches/0001-open-from.patch"
apply "$ROOT/patches/0002-editor-link.patch"
cp "$DIST/toolbox.html" "$DIST/page.template.html"
apply "$ROOT/patches/0003-page-template.patch"

# 5. The manifest.
node "$ROOT/tools/manifest.mjs" "$DIST" "$UPSTREAM" "$TAG" "$COMMIT"
printf 'build.sh: dist/ is turtle-lsp %s (%s) plus our patches\n' "$TAG" "$COMMIT"
ls -l "$DIST"
