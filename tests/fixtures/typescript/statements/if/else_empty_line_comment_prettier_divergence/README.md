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

Per Comment Position Philosophy: preserve user intent when prettier moves a comment.
