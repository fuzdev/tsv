# as_satisfies_value_mixed_comment_prettier_divergence

The mixed-leading extension of
[as_satisfies_value_line_comment](../as_satisfies_value_line_comment_prettier_divergence/):
a redundant paren shell around an `as`/`satisfies` cast type whose leading gap
holds a **block comment before the line comment** (`(/* b */ // c\n A)`, and the
double-nested form).

**tsv**: strips the shell and hangs the whole run at the same fixed point the bare
(paren-free) authoring settles on — the block trails the keyword inline, the line
comment forces the type onto the next line:

```
const a = x as /* b */ // c
	A;
```

**Prettier**: floats the comments out past the whole expression — the block before
the keyword, the line comment to a statement-trailing position:

```
const a = x /* b */ as A; // c
```

Per Comment Position Philosophy, the user wrote the comments inside the cast, so
tsv keeps them associated with the cast rather than floating them past the type
and the statement. The `unformatted_ours_single_paren` /
`unformatted_ours_double_parens` variants verify the paren shells are idempotent
under tsv.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
