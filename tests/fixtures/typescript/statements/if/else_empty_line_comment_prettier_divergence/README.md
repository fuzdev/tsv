# else_empty_line_comment_prettier_divergence

Line comment between `else` keyword and empty statement alternate (`;`).

**Prettier**: relocates the comment before `else`:
```
} // c
else;
```

**tsv**: preserves the comment after `else` (user placement):
```
} else // c
;
```

## Reason

Per the Comment Position Philosophy: preserve user intent when prettier moves a
comment. The fixture also covers non-block and empty consequents before the
empty alternate; prettier relocates the comment before `else` in every case.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
