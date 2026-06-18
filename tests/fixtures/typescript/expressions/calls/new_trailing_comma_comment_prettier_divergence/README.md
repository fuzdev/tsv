# new_trailing_comma_comment_prettier_divergence

Block comment trailing the last `new` argument's comma (`bbb..., /* c */`). The
`new`-expression counterpart of the call-argument
[trailing_comma_comment](../trailing_comma_comment_prettier_divergence/) divergence —
`new` shares the same comment-position rules across every argument path.

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

Only the **last** argument diverges, and only in multiline form — the long names
here force the argument list to expand, keeping the trailing comma.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
