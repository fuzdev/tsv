# trailing_comma_comment_prettier_divergence

Block comment trailing the last object property's comma (`b: 2, /* c */`).

**Prettier**: relocates the comment before the comma (`output_prettier.svelte`):

```
b: 2 /* c */,
```

**tsv**: preserves the comment after the comma:

```
b: 2, /* c */
```

Per Comment Position Philosophy, the user's chosen position is preserved. Both
positions are dual-stable in our formatter.

Only the **last** property diverges — a comment after a non-last property's
comma attaches as the next property's leading comment (both formatters agree).
The divergence is also multiline-only: inline objects drop the trailing comma,
so both formatters converge on `{ b: 2 /* c */ }`.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
