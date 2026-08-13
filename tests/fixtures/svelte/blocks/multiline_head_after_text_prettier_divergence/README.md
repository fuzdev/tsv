# multiline_head_after_text

A control-flow block that **renders multiline** never welds its head to the end of a
preceding text line: `text1 {#if cond}` becomes `text1⏎{#if cond}`. The space between
sibling text and a block head is inter-sibling whitespace — significant in *presence*,
free in *kind* (a space and a newline both render as one space) — so moving the break
there is render-safe, and it is the same posture every other multiline unit already
gets: a multiline inline element or component after spaced text starts on a fresh line
(`break_before_wide_flow` / `flow_forced_break`), a `{#snippet}` takes its own line as
a declaration, and every **non-text** predecessor (element, tag, comment, another
block) already puts a multiline block head on a fresh line — with prettier agreeing.
This fixture closes the one remaining cell: spaced **text** before the four rendering
heads (`{#if}` / `{#each}` / `{#key}` / `{#await}`), in every fragment family — block
element, inline element, component, root, and branch interiors.

Prettier keeps the welded head stable (`prettier_variant_welded`), one form per
authoring; tsv converges both authorings onto `input.svelte`. A block that renders
**inline** still packs into the fill (`text8 {#if b}text9{/if} text10` — the control
case), so the drop keys on the block's rendered layout, not on its presence.

A **glued** head (`text1{#if cond}`) is out of scope: that boundary is
render-significant (splitting it would inject a rendered space), so the weld is kept —
see `../../elements/root_text_control_flow_adjacent/`.

See [conformance_prettier_svelte.md §Svelte: Blocks](../../../../../docs/conformance_prettier_svelte.md#svelte-blocks)
and [§Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
