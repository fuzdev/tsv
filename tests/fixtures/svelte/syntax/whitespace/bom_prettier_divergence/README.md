# bom_prettier_divergence

tsv strips UTF-8 BOM (`0xEF 0xBB 0xBF`). Prettier preserves it.

tsv: strips BOM (parser skips; the formatter never emits a BOM with nothing load-bearing
behind it — the one it writes, ahead of a content U+FEFF, is
[leading_zwnbsp](../leading_zwnbsp_prettier_divergence/README.md))
Prettier: preserves BOM if present

## Reason

BOM stripping. A BOM ahead of ordinary content is meaningless for UTF-8 (legacy artifact from UTF-16 LE/BE detection); the one it carries meaning for, a BOM ahead of a content U+FEFF, is kept (see [leading_zwnbsp](../leading_zwnbsp_prettier_divergence/README.md)). It causes problems with shebang scripts and some tools. Many modern formatters strip BOM (deno fmt, VS Code, etc.).

See [conformance_prettier.md §Whitespace: BOM Handling](../../../../../../docs/conformance_prettier.md#whitespace-bom-handling).

CSS and TypeScript BOM fixtures reference this README.

## The parse pin

`expected_prettier_variant_bom.json` pins the parse of the BOM-led `prettier_variant_bom.svelte`
(P4) — the input itself cannot carry this BOM, which has nothing load-bearing behind it, since
tsv's format strips it and the input must be its own fixed point (F1). It holds Svelte's own
AST of that variant: Svelte's `parse` strips the BOM before parsing, so every offset indexes
the BOM-less string — the `<script>` on line 1, its
`Program` loc, the template `name_loc`, the `<style>` sheet, and the `{a}` island all sit one
UTF-16 unit below their file positions, and the line-1 columns one lower. The CSS sibling pins
`parseCss`'s identical reading; the TypeScript sibling pins acorn's opposite one (the BOM counts
as whitespace, so file coordinates are kept).
