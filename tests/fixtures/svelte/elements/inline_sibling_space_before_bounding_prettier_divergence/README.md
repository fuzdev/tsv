# inline_sibling_space_before_bounding_prettier_divergence

**The follower never turns a space into a newline either.** A space before a **comment**, a
`{@debug}` or a control-flow block that renders inline is that follower's own per-width wrap after
*any* sibling — an inline element, a component, every tag kind, another comment, a `<br />`, an
inline-rendering block, and a control-flow block or element that renders multiline — exactly as it
already is after text, so `<span>inline1</span> <!-- c -->`, `{/if} <!-- c -->`,
`{expr} {#if cond}text1{/if}`, `<span>inline1</span> {#await promise}text1{/await}` and
`<!-- c1 --> <!-- c2 -->` keep their authored space in a multiline container, in a block and at the
root, and a comment spaced on both sides keeps both. Prettier keeps the space after text and breaks
it with the container after anything else (`output_prettier.svelte`). The **newline** spelling of
every such boundary is held by both formatters (`variant_newline.svelte`): a comment's line is
authorship, and it is the newline that holds it.

The controls agree with prettier: a control-flow block that renders **multiline** drops to a fresh
line whole (the wrap's group breaks on the block's own hardlines — the spaced whole-unit drop every
unit kind has), a **block element** predecessor keeps the follower off its line, and a `<br />`
after a comment takes its wrap; the comment's and the inline block's own width boundary hugs at
exactly 100 chars and drops at 101. `unformatted_ours_boundary_space` spells those breaking
boundaries as a space, and `unformatted_ours_spaces` widens every kept space to three; tsv
normalizes both to `input.svelte` in one pass.

`variant_prettier_split.svelte` is prettier's form itself (it differs from `variant_newline` only in keeping
the two after-text spaces), and tsv keeps it stable too, so a rule change that started normalizing
prettier's form back to `input.svelte` fails this file rather than passing in prose.

## Reason

Design choice — the follower-side twin of
[inline_tag_pair_space_bounded](../inline_tag_pair_space_bounded_prettier_divergence/). The
whitespace-only separator's bare `line` before a run-ending follower broke with the container while
the fill's `line` before the same follower packed per width, so `<span>a</span> <!-- c -->` split
where `text1 <!-- c -->` hugged: one boundary, two answers, keyed on whether a text node happened to
precede it. The rule family's thesis is that the prose gate holds an authored newline and never
forces one — a space is never turned into a newline the author did not write — and comment
position is held by the newline spelling, not by breaking the author's space
([§Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)).
The old bare `line` was also half of a two-arm disagreement (the inline arm already wrapped these
followers per width), which is a two-pass cycle rather than a policy; both arms now wrap.

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
