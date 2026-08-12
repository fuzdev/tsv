# jsdoc_type_cast_enum_member_break_prettier_divergence

An enum member's `=` is a value gap, so a JSDoc cast there answers like every other value:
tsv keeps the comment glued to its `(` and the parens preserved, from either authoring —
the mid-line-break authoring reflows back onto the comment's line.

The site is TypeScript-only, and prettier differs **twice** here, so the fixture carries
both:

1. **The parens.** Prettier's TypeScript parser is cast-unaware and strips a JSDoc cast's
   parens, where tsv preserves them (`/** @type {T} */ expr` is not a cast). Nothing new —
   the standing §JSDoc / paren semantics divergence, pinned in
   [`jsdoc_type_cast_ts`](../jsdoc_type_cast_ts_prettier_divergence/) — but it is what makes
   `input.svelte` prettier-unstable, hence the `output_prettier.svelte`.
2. **The break.** From `unformatted_ours_break.svelte` tsv reflows the `(` back onto the
   comment's line while prettier keeps the break where the author put it — the same split
   as at the binding defaults, whose JS context lets that difference stand alone:
   [`jsdoc_type_cast_binding_default_break`](../jsdoc_type_cast_binding_default_break_prettier_divergence/).

The two compose into a form no single-difference marker describes.
`divergent_variant_break.svelte` is prettier-stable (break kept, parens gone) and tsv
rewrites it to a **third** stable form: once the parens are stripped there is no cast left,
only an ordinary block comment whose break the enclosing group decides by width — so the
short member collapses and the wide one keeps its line, which is neither `input.svelte` nor
`output_prettier.svelte`. Three stable forms, which is what `divergent_variant_*` names.

`A` is the control — short enough that the parens stay flat either way. `B` carries the
claim: only past the print width does the group have a break to take, so only there does
the value gap's rule (a space, the parens expanding under it) differ from letting width
decide (the comment keeping its line and the `(` stranded at the member indent).

`C` is the third authoring, and the one the reflow does **not** touch: a comment the author
gave a line of its own hangs the value, the `=` ending its line. The cast prints a hardline
between that comment and its `(`, so the member has to supply the matching hang or the `(`
lands at the member's own indent and the next pass collapses it — an authoring with no fixed
point. The binding defaults take the same rule through the same narrow predicate, and their
own-line authoring is a fixture of its own because prettier relocates the comment there:
[`jsdoc_type_cast_binding_default_own_line`](../jsdoc_type_cast_binding_default_own_line_prettier_divergence/).

## Reason

**Design choice**, twice over. The break is unforced — a block comment does not run to
end-of-line, so nothing pushes the `(` off the comment's line — and tsv reflows an unforced
break at every value position, the enum member named among them
([conformance_prettier.md §Authored breaks in value position](../../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position)).
Preserving the break instead is prettier's standing
[§Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
difference in its value-position form. The paren half is the semantic one: dropping a cast's
parens drops the assertion, so tsv preserves them in every context while prettier's TS parser
does not.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
and [§JSDoc / paren semantics](../../../../../../docs/conformance_prettier_ts_comments.md#jsdoc--paren-semantics).
