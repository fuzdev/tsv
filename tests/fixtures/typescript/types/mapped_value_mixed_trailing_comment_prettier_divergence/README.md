# mapped_value_mixed_trailing_comment_prettier_divergence

The mixed / trailing extension of
[mapped_value_line_comment](../mapped_value_line_comment_prettier_divergence/): a
redundant paren shell around a mapped-type value (`[K in T]: (…)`) whose leading
gap holds a **block before the line comment** (mixed), or whose trailing gap holds
a **block after the value** (trailing).

**tsv**: strips the shell and hangs the run at the same fixed point the bare
authoring settles on — the block trails `:` inline, the line comment forces the
value onto the next line, and a trailing block trails the value before the `;`:

```
type M = {
	[K in Keys]: /* b */ // c
		A;
};
type N = {
	[K in Keys]: // c
		B /* t */;
};
```

**Prettier**: breaks the `[K in Keys]` brackets and trails the comment after the
key type instead:

```
type M = {
	[
		K in Keys /* b */ // c
	]: A;
};
```

Per Comment Position Philosophy, tsv keeps the comments after `:`, associated with
the value. The `unformatted_ours_*` variants verify the paren shells are
idempotent under tsv.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
