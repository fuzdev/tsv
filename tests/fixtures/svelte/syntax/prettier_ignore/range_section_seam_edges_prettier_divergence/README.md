# range_section_seam_edges_prettier_divergence

The cut that lifts a hoisted `<script>` / `<style>` / `<svelte:options>` out of an ignore range
(see [range_section_seam](../range_section_seam_prettier_divergence/)) where more than one run
competes, where a range marker or a comment is the neighbour, and on the second path that makes
the cut.

**Sections in a row are one seam.** Two sections with only whitespace between them leave one
pair of neighbours, and every whitespace run in the seam competes — the one before the first
section, the ones between sections, the one after the last — by the same strength (a blank line,
then a line break, then any other whitespace, then none). A run before or after the sections
that wins is kept as the author wrote it; a run between them that wins prints as a plain space or
line break, never as a blank line — a blank line between two sections is the sections' own, as
outside a range. On a tie a run before or after the sections wins over one between them.

- `unformatted_ours_two_sections` — nothing between the sections, a space before them: the space
  survives (`x <script>…</script><style>…</style>y` → `x y`)
- `unformatted_ours_two_sections_space_between` — a space between the sections only: a space
  (`x<style>…</style> <script>…</script>y` → `x y`)
- `unformatted_ours_between_blank` — a blank line between the sections only: a line break
  (`text1<style>…</style>⏎⏎<script>…</script>text2` → `text1⏎text2`)
- `unformatted_ours_between_tab` — a tab between the sections only: a space
  (`text2<style>…</style>⇥<script>…</script>text3` → `text2 text3`)
- `unformatted_ours_between_newline_beats_space` — a space before the sections, a line break
  between them: the line break wins (`text3 <script module>…</script>⏎<style>…</style>text4` →
  `text3⏎text4`)

**Other neighbours.**

- `unformatted_first_last_in_range` — a range marker is the neighbour: the section written first
  in a range (after `<!-- prettier-ignore-start -->`, itself glued to `text1`) and the one
  written last (before `<!-- prettier-ignore-end -->`) leave the line break written before them.
  Prettier agrees here, so this variant normalizes under both tools
- `unformatted_ours_comment_neighbour` — a comment inside a range stays there (see
  [range_interior_comment](../range_interior_comment_prettier_divergence/)), so it is the
  neighbour, and the space after it survives (`x<!-- k --> <script>…</script>y` →
  `x<!-- k --> y`)

**The rewrite path.** `unformatted_ours_lifted_rewrite` also holds a section written between two
template nodes OUTSIDE any range, which sends the whole document through the source rewrite that
joins such neighbours. A section inside a range is cut the same way on that path: the space before
it survives (`x <script>…</script>y` → `x y`), and a line break before a section beats a space
after it (`{c}⏎<style>…</style> <!-- prettier-ignore-end -->` → `{c}⏎<!-- prettier-ignore-end -->`).

Prettier emits an invalid component on every `unformatted_ours_*` variant: it hoists each section
and leaves a copy of it in the range, which Svelte rejects as a duplicate (`script_duplicate`);
the copy also carries the placeholder prettier holds the section's code in
(`<script ✂prettier:content✂="…">{}</script>`), and prettier's own second pass throws on it
(`Attributes need to be unique`). `input.svelte` holds no section in a range, and both tools keep
it.

## Reason

A hoisted section renders nothing where it was written, so the whitespace around it is all that
separates its neighbours — deleting the only run the author wrote welds them (`x y` renders with
its space, `xy` without, and `text1` glued to a range whose first section had a line break before
it renders `text1 {b}`). Two sections in a row are one gap between the same two neighbours, read
as if the sections had never been there, as the join outside a range reads it
([conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering)),
and the range's own markers and comments are neighbours like any other node.

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
