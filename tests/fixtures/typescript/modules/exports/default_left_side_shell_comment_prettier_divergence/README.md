# default_left_side_shell_comment_prettier_divergence

An own-line comment inside the grouping parens of an `export default` value's **leftmost**
node (`export default (⏎// c⏎a as any).b`). The value's printer hoists the run ahead of its
doc, and the keyword→value seam hangs the value under the keyword — the same hang a
comment authored in the keyword's own gap takes
([§Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)),
and the one-pass fixed point: left on the keyword's line, the run rendered flush and the
reparse indented it.

tsv (`input.svelte`):

```ts
export default
	// c
	(a as any).b;
```

Prettier keeps the comment on the keyword's line and the value flush (`output_prettier.svelte`),
from either authoring:

```ts
export default // c
(a as any).b;
```

The `export default` carve-out of the continuation-indent rule — prettier keeps the
operand break here, tsv hangs it indented. See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Assignment value's left-side shell comment).
