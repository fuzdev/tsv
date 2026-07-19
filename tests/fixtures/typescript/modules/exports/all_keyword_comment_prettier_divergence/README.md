# all_keyword_comment_prettier_divergence

Comments in an export-all (`export * from`) header — between `export`, an optional
`type` keyword, `*`, and `from` — are preserved where the user placed them.

**Prettier**: relocates every header comment to after `from`, before the source
(`output_prettier.svelte`):

```
export * from /* c1 */ './a';
export type * from /* c2 */ './b';
export type * from /* c3 */ './c';
export type * from /* c4 */ './d';
```

**tsv**: preserves each comment where the user placed it:

```
export /* c1 */ * from './a';
export /* c2 */ type * from './b';
export type /* c3 */ * from './c';
export type * /* c4 */ from './d';
```

Per Comment Position Philosophy, the user's chosen position is preserved. Unlike the
default/namespace import headers (prettier keeps comments near the binding), every
export-all header comment relocates after `from`.

**Authoring axis.** `unformatted_ours_own_line.svelte` writes each of these block
comments on its *own* line; tsv reflows every one back to the header line, because a
module header is a keyword→value gap and a single-line block collapses from any
authored position ([§Authored breaks in value
position](../../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position)).
Prettier instead keeps the break *and* relocates past `from`, a form it holds stable
while tsv rewrites it to a third stable form — hence `divergent_variant_own_line.svelte`.
The `export *`/`export type *` gaps are the ones where tsv used to preserve that break,
making them the lone header gaps out of step with `import … from`.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
