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

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
