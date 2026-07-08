# return_type_close_paren_own_line_block_comment_prettier_divergence

A single-line block comment in the `)`→return-type-`:` gap, authored on its own
line (`unformatted_ours_own_line`), normalizes differently in the two formatters:

- **tsv** collapses it inline to `input.svelte` (`f(a) /* c */ : T`), keeping the
  comment where the author parked it — trailing `)`.
- **Prettier** relocates it **into the parameter list** over two passes:
  first-pass `prettier_intermediate_to_variant_own_line` (`f(a) /* c */⏎: T`),
  converging on the second pass to `variant_comment_in_params`
  (`f(a /* c */): T`).

`variant_comment_in_params` is **dual-stable** (both formatters keep it — tsv does
not pull the comment back out of the params), so it is a `variant_*`, not a form
tsv normalizes away. The collapsed `input.svelte` is likewise dual-stable, so
there is no `output_prettier.*`.

## Reason

At a `)`→return-token gap tsv collapses a block comment to the inline form
whenever it can do so losslessly: the own-line placement of a single-line block
carries no signal the inline form drops. This is the same collapse tsv applies at
every keyword→value single-slot gap (`as`/`extends`/`keyof`/…) and at the
function-type `)`→`=>` gap, which behaves identically. Prettier instead treats the
comment as belonging to the last parameter and moves it inside the parens.

A *line* comment in the same gap still hangs the `:` onto the next line — there
inlining would swallow the return type
([return_type_close_paren_line_comment](../return_type_close_paren_line_comment_prettier_divergence/)).

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
