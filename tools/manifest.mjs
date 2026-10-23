// Writes dist/MANIFEST.jsonld: what dist/ holds and where it came from, as
// JSON-LD with its @context inline. Every node has a stable @id (no blank
// nodes): the manifest is urn:toolbox:manifest, each file is the name the
// crate serves it under (urn:toolbox:{name}), and the source is the upstream
// commit's own URL. Digests are TAGGED (`sha256-<hex>`), never multihash.
//
// Terms, all existing: dcterms (hasPart, identifier, license, hasVersion,
// isVersionOf, title, description), prov:wasDerivedFrom, and ik:mediaType /
// ik:sizeBytes ("the media type / size of the representation the subject
// holds", no rdfs:domain, so they type nothing).
//
// usage: node tools/manifest.mjs <dist> <upstream repo URL> <tag> <commit>
import { createHash } from "node:crypto"
import fs from "node:fs"
import path from "node:path"

const [dist, upstream, tag, commit] = process.argv.slice(2)
if (!commit) {
  console.error("usage: node tools/manifest.mjs <dist> <upstream> <tag> <commit>")
  process.exit(2)
}

// The media type each name fixes: the same table the crate serves them with
// (src/lib.rs MEDIA_TYPES), checked against this file by tests/smoke.mjs.
const FILES = {
  "toolbox.html": "text/html; charset=utf-8",
  "page.template.html": "text/html; charset=utf-8",
  "app.js": "text/javascript",
  "samples.js": "text/javascript",
  "worker.js": "text/javascript",
  "engine.js": "text/javascript",
  "style.css": "text/css",
  "tokens.css": "text/css",
  "engine.wasm": "application/wasm",
  "LICENSE.turtle-lsp": "text/plain; charset=utf-8",
}
// What each file is, beyond its upstream origin.
const WHAT = {
  "toolbox.html": "upstream web/app/toolbox.html with patches/0002-editor-link.patch and 0003-input-names.patch",
  "page.template.html":
    "toolbox.html (as patched) made an ikigai-fn compose template by patches/0004-page-template.patch: urn:toolbox:template:page",
  "app.js": "upstream web/app/app.js with patches/0001-open-from.patch",
  "samples.js": "upstream web/app/samples.js",
  "worker.js": "upstream web/app/worker.js",
  "engine.js": "upstream web/app/engine.js",
  "style.css": "upstream web/app/style.css",
  "tokens.css": "ikigai-toolbox's own: upstream's custom properties mapped onto the shared token names (web/tokens.css)",
  "engine.wasm": "upstream web/engine built for wasm32-unknown-unknown by build.sh (cargo build --release --locked)",
  "LICENSE.turtle-lsp": "upstream LICENSE: the copyright and permission notice every copy carries",
}

const commitUrl = `${upstream}/commit/${commit}`
const MIT = "https://spdx.org/licenses/MIT"
const files = Object.keys(FILES).map((name) => {
  const bytes = fs.readFileSync(path.join(dist, name))
  const node = {
    "@id": `urn:toolbox:${name}`,
    title: name,
    description: WHAT[name],
    mediaType: FILES[name],
    sizeBytes: bytes.length,
    identifier: `sha256-${createHash("sha256").update(bytes).digest("hex")}`,
  }
  if (name !== "tokens.css") {
    node.wasDerivedFrom = commitUrl
    node.license = MIT
  }
  return node
})

const manifest = {
  "@context": {
    dcterms: "http://purl.org/dc/terms/",
    prov: "http://www.w3.org/ns/prov#",
    ik: "https://ikigai-rs.dev/ns#",
    xsd: "http://www.w3.org/2001/XMLSchema#",
    title: "dcterms:title",
    description: "dcterms:description",
    identifier: "dcterms:identifier",
    version: "dcterms:hasVersion",
    hasPart: { "@id": "dcterms:hasPart", "@type": "@id" },
    license: { "@id": "dcterms:license", "@type": "@id" },
    isVersionOf: { "@id": "dcterms:isVersionOf", "@type": "@id" },
    wasDerivedFrom: { "@id": "prov:wasDerivedFrom", "@type": "@id" },
    mediaType: "ik:mediaType",
    sizeBytes: { "@id": "ik:sizeBytes", "@type": "xsd:nonNegativeInteger" },
  },
  "@graph": [
    {
      "@id": "urn:toolbox:manifest",
      title: "The RDF Toolbox as ikigai-toolbox serves it",
      description: `turtle-lsp ${tag} (${commit}), built and patched by ikigai-toolbox's build.sh`,
      wasDerivedFrom: commitUrl,
      hasPart: files.map((f) => f["@id"]),
    },
    {
      "@id": commitUrl,
      title: `turtle-lsp ${tag}`,
      identifier: commit,
      version: tag,
      isVersionOf: upstream,
      license: MIT,
    },
    ...files,
  ],
}
fs.writeFileSync(path.join(dist, "MANIFEST.jsonld"), JSON.stringify(manifest, null, 2) + "\n")
console.log(`manifest.mjs: ${files.length} files described in ${path.join(dist, "MANIFEST.jsonld")}`)
