# open_paren_comment_prettier_divergence

A line comment trailing a call's opening `(` on the same line (`fn( // c`) is
preserved on the `(` line, and so is a block comment that shares that line with
something forcing it (`fn( /* paren */` above an own-line `// lead`, or above an
own-line `/* lead */`, which forces the list open just the same — a lone argument
and several alike). Prettier relocates the run to its own line as the first
argument's leading comment.

A block comment alone on the `(` line is not this divergence: it forces nothing,
so it leads the first argument — on the argument's line when the call fits, on a
line of its own above it when the call breaks — the same bytes prettier prints
([first_arg_leading_comment_breaking_value](../first_arg_leading_comment_breaking_value/)).

tsv: keeps the comment trailing `(` where the user placed it
Prettier: moves the comment down to its own line

```
// tsv                          // prettier
fn( // c1                       fn(
	a                                  // c1
);                                a
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

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
