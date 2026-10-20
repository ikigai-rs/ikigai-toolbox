// The smoke test of dist/: the engine as build.sh built it, loaded in Node the
// way the page's worker loads it (engine.js + engine.wasm; the engine needs only
// WebAssembly, crypto.getRandomValues and Date.now), asked what the toolbox's
// tabs ask it; then the manifest against the files, and each patch's mark.
//
// The engine's operations are `version`, `analyse`, `fix_all` and the editor's
// `lsp*`. Validate, Format, SHACL and SPARQL are all faces of ONE `analyse`
// answer: SHACL results are diagnostics on the data file when a file in the
// input declares shapes, and a `.rq` file is analysed as SPARQL.
//
// usage: node tests/smoke.mjs   (after ./build.sh)
import assert from "node:assert/strict"
import { createHash } from "node:crypto"
import fs from "node:fs"
import path from "node:path"
import { fileURLToPath } from "node:url"

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..")
const dist = path.join(root, "dist")
const fixture = (name) => ({ name, text: fs.readFileSync(path.join(root, "tests", "fixtures", name), "utf8") })
const { createEngine } = await import(path.join(dist, "engine.js"))
const engine = await createEngine(fs.readFileSync(path.join(dist, "engine.wasm")))

let failed = 0
function check(title, body) {
  try {
    const detail = body()
    console.log(`ok   ${title}${detail ? `: ${detail}` : ""}`)
  } catch (e) {
    failed++
    console.log(`FAIL ${title}\n     ${String(e.message).split("\n").join("\n     ")}`)
  }
}
const analyse = (...names) => engine.call("analyse", { files: names.map(fixture) })
const diagnosticsOf = (answer, name) => answer.files.find((f) => f.name === name).diagnostics
const brief = (d) => `${d.code} at ${d.range.start.line + 1}:${d.range.start.character + 1}: ${d.message}`

check("version", () => {
  const { version } = engine.call("version")
  assert.equal(version, "0.5.2")
  return `turtle-lsp ${version}`
})

check("validate: a valid Turtle file has no problems", () => {
  const answer = analyse("valid.ttl")
  assert.deepEqual(diagnosticsOf(answer, "valid.ttl"), [])
  assert.equal(answer.hasShapes, false)
  return "0 problems"
})

check("validate: a broken Turtle file has the expected problem", () => {
  const ds = diagnosticsOf(analyse("broken.ttl"), "broken.ttl")
  assert.equal(ds.length, 1, JSON.stringify(ds))
  assert.equal(ds[0].severity, 1)
  assert.equal(ds[0].code, "syntax")
  assert.match(ds[0].message, /Expected '\.' to end the statement/)
  assert.equal(ds[0].range.start.line, 4) // the unterminated `rdfs:comment` line
  return brief(ds[0])
})

check("shacl: a conforming talk message", () => {
  const answer = analyse("talk.shapes.ttl", "message-ok.ttl")
  assert.equal(answer.hasShapes, true)
  assert.deepEqual(diagnosticsOf(answer, "message-ok.ttl"), [])
  assert.deepEqual(diagnosticsOf(answer, "talk.shapes.ttl"), [])
  return "conforms"
})

check("shacl: an unknown property breaks the closed shape", () => {
  const ds = diagnosticsOf(analyse("talk.shapes.ttl", "message-unknown-property.ttl"), "message-unknown-property.ttl")
  assert.equal(ds.length, 1, JSON.stringify(ds))
  assert.equal(ds[0].code, "shacl-violation")
  assert.match(ds[0].message, /dcterms:subject is not allowed .*closed shape <urn:layers:shape:message>/)
  assert.equal(ds[0].range.start.line, 10) // the `dcterms:subject` triple
  return brief(ds[0])
})

check("shacl: a non-IRI ik:inReplyTo", () => {
  const ds = diagnosticsOf(analyse("talk.shapes.ttl", "message-bad-reply.ttl"), "message-bad-reply.ttl")
  assert.equal(ds.length, 1, JSON.stringify(ds))
  assert.equal(ds[0].code, "shacl-violation")
  assert.match(ds[0].message, /ik:inReplyTo .* is not of node kind sh:IRI/)
  assert.equal(ds[0].range.start.line, 7) // the `ik:inReplyTo` triple
  return brief(ds[0])
})

check("sparql: a query with one error", () => {
  const answer = analyse("query-error.rq")
  const file = answer.files.find((f) => f.name === "query-error.rq")
  assert.equal(file.kind, "sparql")
  const errors = file.diagnostics.filter((d) => d.severity === 1)
  assert.equal(errors.length, 1, JSON.stringify(file.diagnostics))
  assert.equal(errors[0].code, "syntax")
  assert.equal(file.format.status, "refused")
  return brief(errors[0])
})

check("manifest: every file, its size and its digest", () => {
  const manifest = JSON.parse(fs.readFileSync(path.join(dist, "MANIFEST.jsonld"), "utf8"))
  const nodes = new Map(manifest["@graph"].map((n) => [n["@id"], n]))
  const top = nodes.get("urn:toolbox:manifest")
  assert.ok(top, "urn:toolbox:manifest is described")
  const onDisk = fs.readdirSync(dist).filter((n) => n !== "MANIFEST.jsonld").sort()
  assert.deepEqual(top.hasPart.map((iri) => iri.replace(/^urn:toolbox:/, "")).sort(), onDisk)
  for (const iri of top.hasPart) {
    const node = nodes.get(iri)
    const bytes = fs.readFileSync(path.join(dist, iri.replace(/^urn:toolbox:/, "")))
    assert.equal(node.sizeBytes, bytes.length, iri)
    assert.equal(node.identifier, `sha256-${createHash("sha256").update(bytes).digest("hex")}`, iri)
  }
  const commit = nodes.get(top.wasDerivedFrom)
  assert.match(top.wasDerivedFrom, /^https:\/\/github\.com\/abhishekkundu25\/turtle-lsp\/commit\/dcfeed02885c012ad5b7d1c28f33151eda8d75f8$/)
  assert.equal(commit.version, "v0.5.2")
  // The crate's fixture (tests/fixtures/MANIFEST.jsonld) is a build of this
  // shape: the same context and the same names, so a change to the generator is
  // a fixture refresh, not a silent drift.
  const sample = JSON.parse(fs.readFileSync(path.join(root, "tests", "fixtures", "MANIFEST.jsonld"), "utf8"))
  assert.deepEqual(sample["@context"], manifest["@context"], "the fixture's @context is the generator's")
  assert.deepEqual(sample["@graph"].map((n) => n["@id"]), manifest["@graph"].map((n) => n["@id"]), "the fixture names what a build names")
  return `${top.hasPart.length} files, sha256 each`
})

check("patches: each one applied", () => {
  const read = (n) => fs.readFileSync(path.join(dist, n), "utf8")
  const app = read("app.js")
  assert.match(app, /\nfunction openFrom\(\) \{/, "0001: the open-from function")
  assert.match(app, /\nif \(!openFrom\(\)\) loadSample\("starter"\)\n$/, "0001: its call")
  const page = read("toolbox.html")
  assert.ok(page.includes('href="https://abhishekkundu25.github.io/turtle-lsp/?editor" rel="noopener"'), "0002: the editor link")
  assert.ok(!page.includes('href="./?editor"'), "0002: no link to an editor we do not serve")
  const template = read("page.template.html")
  assert.ok(template.includes('data-theme="$h{{theme} || urn:toolbox:text:auto}"'), "0003: the theme marker")
  assert.ok(template.includes("data-inputs="), "0003: the inputs marker")
  // D1: values go in with $h only, never $r/$a.
  assert.doesNotMatch(template, /\$[ra]\{/, "0003: the template splices nothing unescaped")
  assert.doesNotMatch(template, /<script>|\sstyle=/, "no inline script or style (D6)")
  return "0001-open-from, 0002-editor-link, 0003-page-template"
})

if (failed) {
  console.log(`\n${failed} check(s) failed`)
  process.exit(1)
}
console.log("\nall smoke checks passed")
