# trailing_comma_comment_prettier_divergence

Block comment trailing the last destructuring element's comma (`b, /* c */`).

**Prettier**: relocates the comment before the comma (`output_prettier.svelte`):

```
bbb... /* c */,
```

**tsv**: preserves the comment after the comma:

```
bbb..., /* c */
```

Per Comment Position Philosophy, the user's chosen position is preserved. Both
positions are dual-stable in our formatter.

Only the **last** element diverges, and only in multiline form — the long names
here force the pattern to expand, keeping the trailing comma. An inline pattern
drops the trailing comma, so both formatters converge on `{ a, b /* c */ }`.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
