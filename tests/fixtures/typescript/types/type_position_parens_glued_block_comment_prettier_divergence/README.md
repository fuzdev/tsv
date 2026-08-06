# type_position_parens_glued_block_comment_prettier_divergence

A single-line block comment glued to the `(` of a **redundant paren shell** the printer
strips, on a non-first item of a type list. The shell is erased, so the comment lands in the
list's own gap — after the comma, leading the next item — and prettier moves it *across* the
comma to trail the previous one.

```
// authored                    // tsv                        // prettier
type B = Foo<T,                type B = Foo<T, /* c */ U>;   type B = Foo<T /* c */, U>;
	(/* c */
	U)>;
```

This is the type-position spelling of the array rule in
[expressions/arrays/end_of_line_block_comment](../../expressions/arrays/end_of_line_block_comment_prettier_divergence/),
reached through a stripped shell rather than an authored newline: prettier classifies on
newlines alone (`endOfLine`), so the comma — which is what carries the association — plays no
part, and the comment written about `U` comes back reading as being about `T`. We preserve the
comment's position: it stays after the comma, leading `U`.

Both positions are dual-stable. `[T, /* c */ U]` and `[T /* c */, U]` are each idempotent
under both formatters (`variant_before_comma`); the divergence is in normalization —
prettier normalizes the shell authoring to before the comma, we normalize it to after
(`unformatted_ours_parens`).

The three positions are one rule, not three: a tuple element, a type argument in type
position, and a type argument in call position, the last two sharing a single expansion
predicate (`type_arguments_force_expansion`). All three ask own-line-ness on the **source**
rather than on the item boundary, because the item spans no longer cover the `(` that
occupied the comment's line — see `OwnLineBasis` in `crates/tsv_ts/src/printer/mod.rs`. On the
boundary reading the list would instead *expand*, a third fixed point neither the bare
authoring nor prettier produces.

The contrast case `C` shows the other side of the comma: a comment the author put before it
(here inside a shell's trailing gap) trails the previous item under both formatters.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
