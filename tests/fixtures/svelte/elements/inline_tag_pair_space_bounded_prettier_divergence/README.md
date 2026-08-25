# inline_tag_pair_space_bounded_prettier_divergence

**The predecessor never decides a space.** A space-spelled tag pair after a comment, a `<br />`,
a control-flow block that renders multiline, or an inline element that renders multiline keeps
its authored space: the separator before the first tag is that tag's own per-width wrap, exactly
what an inline element or component takes at the same boundary (`<!-- c --> <span>x</span>` and
`{/if} <span>x</span>` hug under both formatters), so the pair stays on its neighbour's line and
packs. Prettier's bare `line` between a tag and its neighbour breaks with the multiline
container, so it splits every one of them (`output_prettier.svelte`). The **newline** spelling of
each boundary is held — the sibling-newline flow rule excludes those neighbours — so the two
spellings are two fixed points, the dual stability `variant_space` carries everywhere else.

The follower does not decide it either: the pair keeps its space before a comment and before an
inline-rendering control-flow block, which take that space as their own per-width wrap
([inline_sibling_space_before_bounding](../inline_sibling_space_before_bounding_prettier_divergence/)).
Only two things still break a space, under both formatters: a **block element** predecessor, whose
own break keeps the tag off its line, and a **declaration tag**, which takes its own line by its
own rule. A spaced `<br />` between two tags stays on one line: it is inline flow content on both
of its sides.

`unformatted_ours_line_owner_space.svelte` spells each of those breaking separators as a space;
tsv breaks them back and keeps every pair packed, normalizing to `input.svelte` in one pass,
while prettier splits the pairs as well (`divergent_variant_line_owner_space.svelte`).

`variant_split.svelte` is prettier's form itself, and tsv keeps it stable too — the newline spelling of
every boundary is held — so the two forms are two fixed points; a rule change that started
normalizing prettier's form back to `input.svelte` fails this file rather than passing in prose.

## Reason

Design choice — the rule of
[inline_tag_pair_space](../inline_tag_pair_space_prettier_divergence/) with its last gate removed:
a space before a tag asks nothing about what precedes it, as a space before an inline element or
component never did. Gating it on the predecessor's kind was the one place a space still turned
into a newline the author did not write — a forced break, in a rule family whose whole thesis is
that the prose gate holds an authored newline and never forces one — and it was prettier's
`line`-between-tags rule surviving for one neighbour class after being dropped for the rest. A
comment's line is authorship
([§Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)),
and that is exactly what the NEWLINE spelling holds; an authored space beside the comment is the
author's too, and tsv already kept it for text, an element and a component. The boundary set is
the flow rule's own
([inline_sibling_newline_run_bounded](../inline_sibling_newline_run_bounded_prettier_divergence/)
holds the newline spellings across it); the multiline-predecessor pair is the tag twin of
[inline_adjacent_component_after_multiline](../inline_adjacent_component_after_multiline_prettier_divergence/).

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
