# trailing_comma_comment_prettier_divergence

Block comment trailing the last function-type param's comma (`b: number, /* c */`).

**Prettier**: relocates the comment before the comma (`output_prettier.svelte`):

```
b: number /* c */,
```

**tsv**: preserves the comment after the comma:

```
b: number, /* c */
```

Per Comment Position Philosophy, the user's chosen position is preserved. Both
positions are dual-stable in our formatter.

Only the **last** param diverges, and only in multiline form — the long names
here force the parameter list to expand, keeping the trailing comma.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
