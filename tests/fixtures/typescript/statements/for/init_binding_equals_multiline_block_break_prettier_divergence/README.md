# init_binding_equals_multiline_block_break_prettier_divergence

A **multiline** block comment in a for-init binding's `=` gap that the author
**broke after** (`for (let i /* x⏎y */⏎= 0; …)`). The break after a multiline block
is authoring signal — the same rule the value gap applies (`= /* x⏎y */⏎0` hangs in
both formatters) — so tsv keeps it: the comment trails the binding and `= value`
drops to a continuation line **indented one level** (the uniform forced-continuation
indent, the same landing as the line-comment sibling
[init_binding_equals_line_comment](../init_binding_equals_line_comment_prettier_divergence/)).
Prettier instead **relocates** the comment across the `=` and hangs it leading the
value (`output_prettier.svelte`).

```
// tsv (preserve + continuation indent)   // prettier (relocate past `=`, hang)
for (                                     for (
	let i /* x                              let i =
y */                                      		/* x
		= 0;                              y */
	i < 10;                                   	0;
	i++                                       i < 10;
) {}                                        i++
                                          ) {}
```

The comment's own interior lines stay flush in both — neither formatter re-indents
a comment body. One whose value shares its closing line (`let f /* x⏎y */ = 0`) is
glued in both formatters and is pinned as a plain case of
[init_binding_equals_comment](../init_binding_equals_comment/), so only the
broke-after authoring diverges.

The for-init face of the variable-declarator
[declarator_before_eq_multiline_block_break](../../../declarations/variable/declarator_before_eq_multiline_block_break_prettier_divergence/),
whose gap this one answers identically.

See
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation
and [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Uniform Forced-Continuation Indent.
