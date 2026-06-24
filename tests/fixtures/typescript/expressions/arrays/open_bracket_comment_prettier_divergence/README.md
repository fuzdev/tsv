# open_bracket_comment_prettier_divergence

A comment trailing an array literal's opening `[` on the same line (e.g.
`[ // c` or `[ /* c */`) is preserved on the `[` line. Prettier relocates it to
its own line as the first element's leading comment.

tsv: keeps the comment trailing `[` where the user placed it
Prettier: moves the comment down to its own line

```
// tsv                          // prettier
const a = [ // c1               const a = [
	x                                  // c1
];                                 x
                                 ];
```

## Reason

tsv treats user comment placement as intentional (see Comment Position
Philosophy). A comment the author parked after `[` is a trailing comment on
that line; relocating it to its own line is a syntactic-position move. tsv
preserves it in place, which is also idempotent in a single pass (Prettier's
relocation is its own canonical form). When the author instead writes the
comment on its own line, both formatters keep it there — the two positions are
dual-stable. Inline block comments that hug content in an array that stays
inline (`[/* c */ x]` on one line) are unchanged and match Prettier; only the
expanding cases (a line comment after `[`, or own-line content forcing a break)
diverge.

Consistent with tsv's handling of the same comment position after a call's
opening `(`
([open_paren_comment](../../calls/open_paren_comment_prettier_divergence/))
and an object literal's opening `{`
([open_brace_comment](../../objects/open_brace_comment_prettier_divergence/)).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
