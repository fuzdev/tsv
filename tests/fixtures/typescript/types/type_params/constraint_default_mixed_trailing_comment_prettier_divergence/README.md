# constraint_default_mixed_trailing_comment_prettier_divergence

The mixed / trailing extension of
[default_paren_leading_line_comment](../default_paren_leading_line_comment/): a
redundant paren shell around a type-parameter `= default` value or `extends`
constraint whose leading gap holds a **block before the line comment** (mixed), or
whose trailing gap holds a **block after the value** (trailing).

**tsv**: strips the shell and hangs the run at the same fixed point the bare
authoring settles on — the block trails the `=`/`extends` keyword inline, the line
comment forces the value onto the next line indented one level, and a trailing
block trails the value:

```
type D<
	U = /* b */ // c
		A
> = U;
type F<
	U extends /* b */ // c
		A
> = U;
```

**Prettier**: reorders the run — the block moves before the `=`/`extends`
(`U /* b */ =`, `U /* b */ extends`), the line comment stays after it, and the
value hangs flush (no continuation indent):

```
type D<
	U /* b */ = // c
		A
> = U;
```

Unlike the plain `default_paren_leading_line_comment` sibling (where both
formatters agree on the pure-line normalization), the block relocation makes this
a divergence. Per Comment Position Philosophy, tsv keeps the comments after the
keyword. The `unformatted_ours_*` variants verify the paren shells are idempotent
under tsv.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
