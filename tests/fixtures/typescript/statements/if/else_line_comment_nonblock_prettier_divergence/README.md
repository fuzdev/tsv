# else_line_comment_nonblock_prettier_divergence

Line comment between `else` keyword and non-block body expression.

**Prettier**: relocates the comment before `else`:
```
} // c1
else expr;
```

**tsv**: preserves the comment after `else` (user placement), body indented:
```
} else // c1
  expr;
```

Per Comment Position Philosophy: preserve user intent when prettier moves a comment.
Both positions are dual-stable (`variant_comment_before_else.svelte`).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
