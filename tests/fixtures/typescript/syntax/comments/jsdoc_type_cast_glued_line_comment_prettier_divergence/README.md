# jsdoc_type_cast_glued_line_comment_prettier_divergence

A **line** comment the author wrote on a JSDoc type cast's `(` line
(`/** @type {A} */ ( // d`). tsv keeps it there; **prettier moves it to its own line**
inside the parens.

```
// tsv                                  // prettier
const v2 = /** @type {A} */ ( // d2     const v2 = /** @type {A} */ (
	x2                                      // d2
);                                        x2
                                        );
```

## Reason

The cast's pair is a retained `(` like `return (` or a call's, and the opening-delimiter
rule answers it the same way: what the author put on an opening delimiter's line stays on
it, and what they put on its own line keeps its own line, so both authorings are fixed
points. The sibling sites are
[keyword_open_paren_line_comment](../keyword_open_paren_line_comment_prettier_divergence/)
(`return (` / `throw (`) and the unary comment-holder
([paren_glued_line_comment](../../../expressions/unary/paren_glued_line_comment_prettier_divergence/)).

The `<script>` carries no `lang`, so prettier reads it as JavaScript (babel), which keeps
the cast's parens — its TypeScript parser drops them, and is no oracle for a cast.

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).

## Cases

- **d1–d2** — the plain shape, at `await` and at a declarator.
- **d3** — only the **glued** comment claims the `(` line; the rest of the run keeps its
  own lines below it. At most one can, a `//` running to end of line.
- **d4** — the author blank below the glued comment survives it.
- **d5–d6** — the blank scan stops at a nested cast's comment, which the nested pair owns:
  **d5**'s blank rides, and **d6**, written without one, gains none.
- **d7** — the rule's bound: a **block** glued to the `(` claims nothing, so a `//` behind
  it on that same line has no line to claim either and the whole run stays below. Prettier
  agrees here. The glued authoring is `unformatted_ours_block_glued.svelte`.
- **d8** — the control: written on its own line the comment keeps its own line, and
  **prettier agrees**. `variant_own_line.svelte` is every cell written that way — prettier's
  output, which tsv holds too, so the two formatters part on preserving the author's
  choice, not on one layout.

## Variants

- `unformatted_ours_block_glued.svelte` — d7 written glued (`( /* e7 */ // d7`); both
  formatters move the run below the `(`, so it normalizes to input under tsv and to
  `output_prettier.svelte` under prettier.
