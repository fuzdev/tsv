# body_blank_break_prettier_divergence

An authored **blank line inside a block body** forces the construct open: the body, its
sections and the `{/tag}` close each take their own line and the blank survives — at every
block kind (`{#if}` / `{#each}` / `{#key}` / `{#await}` / `{#snippet}`), at every section and
branch marker (`{:else}`, `{:else if}`, `{:then}`, `{:catch}`, `{#each}`'s `{:else}`), in a
snippet declared inside a component, and in both spellings the parser gives a blank — a
whitespace-only node between two siblings, or a content text's edge whitespace. So the hugged
and expanded authorings of one document reach one form (`unformatted_ours_hugged.svelte` →
`input.svelte`).

Prettier keeps the blank too, but welds the body to its head and its close while breaking
only at the blank (`{#if cond}<span>inline1</span>⏎⏎\t<span>inline2</span>{/if}`,
`prettier_variant_hugged.svelte`) — a stable form tsv normalizes to `input.svelte` in one
pass.

## Reason

Design choice. A blank line is a Tier-2 authoring signal independent of render, so it is
content structure rather than boundary spelling: the body's own boundary whitespace is
render-free and may not select the layout, but a blank *between two children* is exactly the
kind of fact the body-expand decision already breaks on (consecutive block-element children,
a declaration that owns its line — [block_children_break](../block_children_break_prettier_divergence/)).
Hugging past it is the drop: the body lays out as one flowed run and the fill deletes the
blank. Prettier's half-welded shape is the one tsv's all-or-nothing hug rule exists to
avoid — a render-free boundary character welding one side of the construct while the other
breaks.

## Files

- `unformatted_ours_hugged.svelte` — every case authored hugged to its head and close; tsv
  reaches `input.svelte` in one pass, prettier reaches `prettier_variant_hugged.svelte`. It
  is also where every control that *has* a blank carries it, since a control written without
  one grades nothing.
- `unformatted_boundary_blank.svelte` — air on the body's **own boundary** (after the head,
  before the close, on both sides of a `{:else}` marker). Render-free, so both formatters
  delete it while the interior blank beside it survives: the control that keeps "a blank
  breaks the body" from being satisfied by a rule that never asks *where* the blank is.
- `unformatted_blank_run.svelte` — every interior blank authored as a run of three. Both
  formatters collapse it to the one blank line the signal asks for.

The controls, each isolating one way a blank does *not* count, all hugged under both
formatters:

- a blank **interior to one text** is the fill's own — a run is a partition of nodes, so it
  bounds nothing and the fill collapses it
  ([content_interior_blank_collapse](../../elements/content_interior_blank_collapse/) is that
  answer in full, across every container kind);
- a body with **no blank** keeps the hug the rule must not take away;
- a blank whose run a hoisted `{@debug}` **trims** away at the body's EDGE carries no signal,
  because the run it lived in is deleted — and it is deleted in **either** spelling, so the
  text-neighbour and element-neighbour authorings converge alike
  ([hoisted_boundary_convergence](../hoisted_boundary_convergence_prettier_divergence/) owns
  that answer; prettier keeps that blank as it keeps the others). The `{#each}` twin is what
  says the exclusion is not keyed to `{#if}`. Reading the *compiler's*
  content bounds here — where a whitespace-only node counts as content, since the compiler is
  asked whether a node stands between the content and the edge — split those two spellings:
  `{#if c}<span>x</span>⏎⏎{@debug cond}{/if}` broke its body while
  `{#if c}text1⏎⏎{@debug cond}{/if}` trimmed the blank away, one rule with two answers decided
  by which node the parser folded the whitespace into;
- `<pre>` is whitespace-significant, so the gate never reaches it: its body keeps the authored
  blank *and* the hug, identically under both formatters.

The hoisted node with content on **both** sides is a *case*, not a control, and is what says
the exclusion is the **bounds** rather than the node: there the two runs merge into the one
rendered space instead of vanishing, so the blank beside it is interior content and breaks the
body like any other. The element twin (`<div>a⏎⏎{@debug cond}⏎⏎b</div>`) already answered that
way under both formatters, so a hugged body losing it was this bug and not the trim.

See
[conformance_prettier_svelte.md §Svelte: Blocks](../../../../../docs/conformance_prettier_svelte.md#svelte-blocks).
