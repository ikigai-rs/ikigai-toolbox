# Changelog

## 0.2.0

- Move to ikigai-core 0.2.0, the streams release (ledger #2261). This is a
  breaking dependency update because core types appear in the public API.
- Update ikigai-fn to 0.7.0, ikigai-fs to 0.2.0, and ikigai-rdf to 0.3.0 so
  hosts link a single core 0.2.0.
- Run ikigai-conformance 0.11.0 with DATATYPES and FAILURES enabled by default.
  Waive DATATYPES for the file endpoint because its media type follows the
  filename rather than content negotiation; the manifest's Turtle face remains
  covered by the file tests.
