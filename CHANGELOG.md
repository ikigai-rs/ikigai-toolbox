# Changelog

## 0.2.1

- `urn:toolbox:{name}` passes its file's body through (ledger #2290). The kernel's
  body path (`invoke_body`) hands on the base's answer as it came, relabeled with
  the media type the name fixes, and no longer re-wraps collected bytes. An fs
  `Replay` keeps its validator, its length and its range reads, and a file larger
  than the run's byte budget streams without being held whole. The bytes path
  (`invoke`) keeps its behavior and its bounds: the same answer, collected, and
  `OverBudget` past the budget. `tests/bodies.rs` covers both paths.

## 0.2.0

- Move to ikigai-core 0.2.0, the streams release (ledger #2261). This is a
  breaking dependency update because core types appear in the public API.
- Update ikigai-fn to 0.7.0, ikigai-fs to 0.2.0, and ikigai-rdf to 0.3.0 so
  hosts link a single core 0.2.0.
- Run ikigai-conformance 0.11.0 with DATATYPES and FAILURES enabled by default.
  Waive DATATYPES for the file endpoint because its media type follows the
  filename rather than content negotiation; the manifest's Turtle face remains
  covered by the file tests.
