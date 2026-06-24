# all_source_trailing_comment_prettier_divergence

Comments between an `export *`/`export * as ns` re-export's source literal and
the terminating `;` are preserved where the user placed them.

**Prettier**: keeps a same-line block comment after the source in place, but
relocates a line comment past the `;` (`output_prettier.svelte`):

```
export * from './a' /* c */;
export * as ns from './b'; // c
```

**tsv**: preserves them before `;` — a block comment trails the source, a line
comment stays on its line with `;` following:

```
export * from './a' /* c */;
export * as ns from './b' // c
;
```

Per Comment Position Philosophy (and the before-semicolon divergence), the user's
chosen position is preserved. The block case is dual-stable; only the line comment
diverges. Same mechanism as `imports/source_trailing_comment_prettier_divergence`.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
