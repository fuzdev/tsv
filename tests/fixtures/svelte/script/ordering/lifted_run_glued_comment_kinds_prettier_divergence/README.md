# lifted_run_glued_comment_kinds_prettier_divergence

Every hoisted section kind, each with its travelling comments glued to it, written between two
template nodes glued to it: `<svelte:options>` (`unformatted_lifted_options`), a
`prettier-ignore`-frozen `<script module>` (`unformatted_ours_lifted_ignore`, and with a space on
each side, `unformatted_ours_lifted_ignore_spaced`), an instance
`<script>` led by a run of two comments (`unformatted_ours_lifted_comment_pair`, and between two
paragraphs with a blank line after it, `unformatted_ours_lifted_comment_pair_blank_after`), a
`<style>` (`unformatted_ours_lifted_style`), and two runs in a row, a `<script>` then a `<style>`
(`unformatted_ours_lifted_two_runs`). Each run goes to its canonical position, and the neighbours
join as if it had never been there: glued `x` and `y` stay glued, `xy`, as the source renders,
spaced ones join with a space, and the blank line written after the run stays between the two
paragraphs.

Prettier gets the `<svelte:options>` case right. With a comment glued to a `<script>` or a
`<style>` it drops nodes from the END of the template: the `<style>`'s own leading comment, which
ends the template there (`variant_style_comment_dropped.svelte`, from both comment-pair
variants), the last text (`variant_last_node_dropped.svelte`), or — with two such runs — more of the
template's tail (`variant_last_nodes_dropped.svelte`). A glued `prettier-ignore`, spaced or not, drops the
last node like any glued comment; here the template ends in the `<style>`'s comment, so it loses
no content but opens a blank line between that comment and the tag
(`variant_style_comment_blank.svelte`). Each is prettier's own fixed point.

## Reason

◆content_preservation. See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering).
