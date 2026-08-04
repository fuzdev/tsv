# comma_comment_glued_run_prettier_divergence

Two comments **glued to each other** in the run that opens a declaration's value —
`transition: /* c *//* d */color 0.3s, …`.

The run is between-material (postcss `raws.between`), so when the comma list breaks it is
emitted on the colon's line and the elements break beneath it — tsv and prettier agree on
that much ([comma_comment_interior](../comma_comment_interior/)). They differ on the run's
own interior whitespace:

tsv: joins the run single-spaced (`/* c */ /* d */`)
Prettier: preserves whatever spacing the input has, so the glued form stays glued

Both forms are stable under both formatters, so the divergence is only reachable from the
glued authoring: `input.svelte` is the spaced form both keep, and
`prettier_variant_glued_run.svelte` is the glued form prettier keeps and tsv normalizes to
`input`.

## Reason

Stable quirk. tsv normalizes whitespace around comments consistently, the same rule it
applies after the `:` ([in_property_value_after_colon](../../../tokens/comments/in_property_value_after_colon_prettier_divergence/))
and to a run on the property side of it
([multi_comment_before_colon](../../../tokens/comments/multi_comment_before_colon_prettier_divergence/)).
Spacing is safe here in a way it is not in a selector: a value's between-material carries
no structure, so `/* c *//* d */` and `/* c */ /* d */` tokenize to the same value — unlike
a compound selector, where inserting a space turns `.a/* c */.b` into a descendant
`.a .b` and the run therefore stays glued
([combinator_comment](../../../selectors/combinator_comment_svelte_prettier_divergence/)).

See [conformance_prettier_css.md §CSS: Comments](../../../../../../docs/conformance_prettier_css.md#css-comments).

## Related

- [comma_comment_interior](../comma_comment_interior/) — the run's placement, which both
  formatters agree on
- [comma_comment_only_element](../comma_comment_only_element_prettier_divergence/) — the run
  with no element content to hoist away from
