# grid_template_areas_multirow_comment_run_prettier_divergence

A run of two comments opening a grid multirow value, with a **newline** inside the run —
`grid-template-areas: /* a */⏎/* b */ 'x x'⏎'y z'`.

The run is between-material (postcss `raws.between`), so it stays on the colon's line and the
rows break beneath it — tsv and prettier agree on that much
([grid_template_areas_multirow_comment](../grid_template_areas_multirow_comment/)). They differ
on the run's own interior whitespace:

tsv: joins the run single-spaced (`/* a */ /* b */`) — the newline inside the run is not a row
break, since the run is outside the row decision entirely
Prettier: emits `raws.between` verbatim, so the newline and the authored indent come through
and `/* b */` lands on a line of its own above the rows

Both forms are stable under prettier, so `input.svelte` is the single-spaced form both keep
and `prettier_variant_newline_in_run.svelte` is the form prettier keeps and tsv normalizes to
`input`.

## Reason

Stable quirk. tsv normalizes whitespace around comments consistently, the same rule it
applies after the `:` ([in_property_value_after_colon](../../tokens/comments/in_property_value_after_colon_prettier_divergence/),
whose `prettier_variant_newline` is the one-comment instance of this newline) and to a glued
run opening a comma list ([comma_comment_glued_run](../../values/lists/comma_comment_glued_run_prettier_divergence/)).
See [conformance_prettier_css.md §CSS: Comments](../../../../../docs/conformance_prettier_css.md#css-comments).

## Related

- [grid_template_areas_multirow_comment](../grid_template_areas_multirow_comment/) — the rows a
  comment rides, which both formatters agree on
- [grid_template_areas_multirow](../grid_template_areas_multirow/) — the comment-free rows
