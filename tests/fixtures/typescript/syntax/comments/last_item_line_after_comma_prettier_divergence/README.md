# last_item_line_after_comma_prettier_divergence

A line comment after the **last** item's trailing comma, where the author gave the comma
a line of its own (`b: B⏎, // c⏎)`). Under `trailingComma: 'none'` that comma is deleted,
so tsv keeps the comment on the line the author gave it; prettier hoists it onto the last
item.

```
// input (author's placement)   // tsv (preserve)       // prettier (hoist)
function fn(                    function fn(             function fn(
	a: A,                           a: A,                    a: A,
	b: B                            b: B                     b: B // c
	, // c                          // c                  ) {}
) {}                            ) {}
```

Both formatters agree on tsv's form once it is written that way, so the divergence is a
**normalization** one: `input.svelte` is a shared fixed point and only the authoring in
`unformatted_ours_trailing_comma.svelte` splits them. That is why there is no
`output_prettier.svelte` — prettier reproduces `input.svelte` byte for byte. Prettier's
own answer to that authoring is the second fixed point, pinned as
`variant_trailing_comma.svelte`: both formatters hold it stable too, so the pair is
dual-stable and the only thing that separates them is which one the authoring reaches.

The rule that separates this from the non-last case is the **anchor**. Everywhere else in
a comma-separated list the after-comma anchor is the comma, because the printer pulls that
comma back onto the previous item's line whatever the author did with it — so a comment
written against it trails the item ([param_line_before_comma](../param_line_before_comma/),
[line_comments_around_comma](../line_comments_around_comma/)). A **last** item's comma is
not re-emitted at all, so that argument does not reach it and the comment keeps its own
line, exactly as it does when the author writes no comma
([trailing_param_comment](../trailing_param_comment/) `fn2`, where both formatters agree).

One rule across every comma-separated list that can hold a trailing comma: function and
arrow parameters, function/constructor-type parameters, tuple elements, and
type-parameter declarations, all covered here.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
