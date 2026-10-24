# Divergence: `export default`→first-decorator glued comment on an `abstract` class

A block comment **glued to the first decorator** of a decorated, `abstract`, default-exported
class (`export default⏎/* c */ @dec⏎abstract class {}`) — the two divergences of its siblings
in one input.

**Parser (Svelte):** acorn-typescript rejects an `abstract` modifier between a decorator list
and the `class` of an `export default` value; tsc accepts it, and tsv parses it as the class
expression it builds for the class without `abstract`, with `abstract: true`
(`expected_ours.json`; the canonical parser errors → `expected_svelte.json`). See
[after_export_default_abstract](../after_export_default_abstract_svelte_divergence/).

**Formatter (prettier):** tsv keeps the comment glued to `@dec`, where the parser bound it,
exactly as for the class without `abstract`; prettier relocates it back past `default`
(`export default /* c */⏎@dec`) and then adds one blank line below it per pass, saturating at
two — `output_prettier.svelte` is its first pass and `audit_signature.txt` pins the chain. The
one-line authoring (`unformatted_ours_inline`, `export default /* c */ @dec abstract class {}`)
glues the comment to `@dec` too, so tsv prints it as `input`, while prettier's chain from it
lands on `output_prettier.svelte`. See the class without `abstract`,
[default_decorator_lead_comment](../../../modules/exports/default_decorator_lead_comment_prettier_divergence/).

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §TypeScript
Corrections and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
