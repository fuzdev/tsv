# member_init_multiline_tail_prettier_divergence

A block comment glued to an enum member's `=` and broken after, above a **multi-line** comment
glued to the value.

**tsv** keeps both comments where the author wrote them and the value keeps its own layout
below the multi-line one:

```
A = /* x */
/* y
 */ aaa ? bbb : ccc
```

**Prettier** relocates the first comment to before the `=`:

```
A /* x */ = /* y
 */ aaa ? bbb : ccc
```

That form is `output_prettier.svelte`, and tsv keeps it too, since tsv preserves a comment
wherever the author put it. `unformatted_ours_paren.svelte` spells the value inside grouping
parens with a break after the `(`, which tsv strips and normalizes to input.

The multi-line comment is what makes this more than the single-comment relocation: its
reprinted body cannot print flat, so the break after the first comment is forced, and the comment
prints outside the value's own group. A value that fits, like the conditional here, stays on one
line rather than breaking at every operator.

## Reason

**Comment position.** Where tsv and prettier part is the first comment's position: tsv preserves
it, prettier moves it across the `=`, which is the standing
[§Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
divergence. The break it keeps is the forced break of
[conformance_prettier.md §Authored breaks in value position](../../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position).

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
