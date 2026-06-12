# all_namespace_keyword_comment_prettier_divergence

Comments in a namespace re-export's header (`export * as ns from`) — between `*` and
`as`, around the binding — are preserved where the user placed them.

**Prettier**: relocates a comment between `*` and `as` to after `as`, before the
binding (`output_prettier.svelte`); a line comment stays on its own line after `as`:

```
export * as /* c1 */ ns1 from './a';
export type * as /* c2 */ ns2 from './b';
export * as // c3
ns3 from './c';
export * as /* c4 */ ns4 from './d';
export * as ns5 /* c5 */ from './e';
```

**tsv**: preserves each comment where the user placed it — a block comment trails
inline, a line comment stays on its own line with `as` following:

```
export * /* c1 */ as ns1 from './a';
export type * /* c2 */ as ns2 from './b';
export * // c3
as ns3 from './c';
export * as /* c4 */ ns4 from './d';
export * as ns5 /* c5 */ from './e';
```

Only the `*`→`as` gap diverges (c1, c2, c3); the `as`→binding (c4) and
binding→`from` (c5) positions are dual-stable (both formatters keep them). Per
Comment Position Philosophy. Sibling of the export-all
`all_keyword_comment_prettier_divergence` (no binding, where prettier relocates
after `from`).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
