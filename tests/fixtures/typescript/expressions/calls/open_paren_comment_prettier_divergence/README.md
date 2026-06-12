# open_paren_comment_prettier_divergence

A comment trailing a call's opening `(` on the same line (e.g. `fn( // c` or
`fn(/* c */`) is preserved on the `(` line. Prettier relocates it to its own
line as the first argument's leading comment.

tsv: keeps the comment trailing `(` where the user placed it
Prettier: moves the comment down to its own line

```
// tsv                          // prettier
fn( // c1                       fn(
	a,                                // c1
);                                a,
                                );
```

## Reason

tsv treats user comment placement as intentional (see Comment Position
Philosophy). A comment the author parked after `(` is a trailing comment on
that line; relocating it to its own line is a syntactic-position move. tsv
preserves it in place, which is also idempotent in a single pass (Prettier's
relocation is its own canonical form). When the author instead writes the
comment on its own line, both formatters keep it there — the two positions are
dual-stable.

Consistent with tsv's handling of comments between a keyword and `(`
([keyword_paren_comment](../../../statements/if/keyword_paren_comment_prettier_divergence/)),
do-while `(`/`)` comments
([open_paren_comment](../../../statements/do_while/open_paren_comment_prettier_divergence/)),
and first-argument leading comments across call chains.

See [conformance_prettier.md](../../../../docs/conformance_prettier.md) §Comment relocation.
