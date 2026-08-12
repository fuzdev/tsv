# Divergence: assignment before-operator multiline block, authored break kept

A **multiline** block comment between an assignment's target and its operator that
the author **broke after** (`d /* x⏎y */⏎= 5;`). The break after a multiline block
is authoring signal — the same rule the value gap applies (`= /* x⏎y */⏎5` hangs in
both formatters) — so tsv keeps it: the comment trails the target and the operator
and value drop to a continuation line **indented one level** (the uniform
forced-continuation indent, the same landing as the line-comment sibling
[before_operator_line_comment](../before_operator_line_comment_prettier_divergence/)).
Prettier instead **relocates** the comment across the operator and hangs it leading
the value (`output_prettier.svelte`).

```ts
// tsv (preserve + continuation indent)   // prettier (relocate past `=`, hang)
d /* x                                    d =
y */                                      	/* x
	= 5;                                  y */
                                          	5;
```

One whose operator shares the comment's closing line (`e /* x⏎y */ = 6`) is glued in
both formatters — the unbroken authoring carries no signal — so only the broke-after
form diverges. The comment's own interior lines stay flush in both; neither formatter
re-indents a comment body.

The assignment-expression face of the variable-declarator
[declarator_before_eq_multiline_block_break](../../../declarations/variable/declarator_before_eq_multiline_block_break_prettier_divergence/),
whose gap this one answers identically.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation and
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Uniform Forced-Continuation Indent.
