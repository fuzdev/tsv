# trailing_comma_comment_prettier_divergence

Block comment trailing the last type parameter's comma (`bbb..., /* c */`).

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

Only the **last** type parameter diverges, and only in multiline form — the long
names here force the type-parameter list to expand, keeping the trailing comma.
Generic type **arguments** (`Map<A, B>`) do not keep a trailing comma, so both
formatters converge there (no divergence).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
