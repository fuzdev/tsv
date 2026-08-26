# body_blank_hoisted_edge_kinds_prettier_divergence

An authored blank line at a **hoisted body edge** carries a Tier-2 signal — and forces the body
open like any other interior blank — **unless the printer actually deletes the run it sits in**.
The complement of [body_blank_break](../body_blank_break_prettier_divergence/)'s `{@debug}`
control, at every neighbour kind that control's trim excludes.

Prettier keeps the blank in each case and welds the body to its head and its close
(`prettier_variant_hugged.svelte`), the half-welded shape tsv's all-or-nothing hug rule exists to
avoid; tsv normalizes it to `input.svelte`.

## Reason

Design choice — and the bound on one already made. The hoisted-edge trim is deliberately narrow:
the hoisted end must be a `{@debug}` and the content end a sibling whose own authored newline
flows ([hoisted_boundary_sibling_kinds](../hoisted_boundary_sibling_kinds_prettier_divergence/)).
At every kind it excludes, the run is **not** deleted — so the argument that retires the blank
there ("a deleted run has no boundary left to carry a Tier-2 signal") does not reach, and the
blank is an ordinary interior one.

⚠️ **The exclusion and its argument have to be the same shape.** A blank gate keyed on hoisted
*adjacency* rather than on *deletion* answers "no signal" wherever a hoisted node stands at the
edge, including at every kind the trim declines — and the body then hugs past the blank and its
fill collapses it. That is the drop
[body_blank_break](../body_blank_break_prettier_divergence/) exists to close, surviving at the
kinds its own control does not reach.

⚠️ The split is invisible to a single-pass check, because the **expanded** authoring is a fixed
point either way: the body is already open, so nothing forces it. Only the **hugged** authoring
discriminates — which is why the claim this fixture makes lives in
`unformatted_ours_hugged.svelte`, not in `input.svelte`.

See
[conformance_prettier_svelte.md §Svelte: Blocks](../../../../../docs/conformance_prettier_svelte.md#svelte-blocks).

## Cases

- **a comment as the content end**, at either edge — its position is authorship, so it owns its
  line and the run beside it is that line's separator.
- **a `<br />` as the content end**, at either edge — a rendered line break owns the line beside
  it, the same reading that keeps it out of the flow set.
- **an `{#each}` twin** of the comment case — the exclusion is not keyed to `{#if}`.
- **a `<title>` as the HOISTED end** — it hoists, but it is an element and keeps its line, so the
  trim declines and the run survives. Nested inside `<svelte:head>`, where head context reaches
  through the block and makes `<title>` a `TitleElement`.

  ⚠️ Its **text twin sits beside it and gives the opposite answer**, and that pair is the sharpest
  thing this fixture pins: `{#if cond}text1⏎⏎<title>text2</title>{/if}` welds, blank and all,
  because that run is the content **text's own edge** and `handle_content_text_child` deletes it —
  where the separator run beside the identical `<title>` survives, because
  `is_hoisted_edge_separator` requires the hoisted end to be a `{@debug}`. **The split is the
  EMITTER, not the hoisted kind**, which is exactly why a gate keyed on hoisted adjacency cannot
  express it: one node, one blank, two answers, decided by which node the parser folded the
  whitespace into. A third cell holds the same text spelling inside a `<div>`, where head context
  stops at a `RegularElement` so the `<title>` is an ordinary element, nothing is hoisted, and no
  trim is in play at all — the null control that keeps the pair from reading as a rule about the
  tag name.

## Controls

Each stays hugged, and each isolates one way the blank genuinely carries nothing:

- **the run IS deleted** — every content-end kind whose newline flows (an inline element, a text,
  an expression tag, a component) before a `{@debug}`. Without these the fixture is satisfied by a
  rule that never trims at all.
- **neither end is content** — a `<span>` between two `{@debug}` tags: both runs are deleted, so
  no blank spelling survives to force anything.
- **the body's own boundary air** — render-free at either end, and never the signal.

## Files

- `unformatted_ours_hugged.svelte` — every case authored hugged to its head and close. tsv reaches
  `input.svelte` in one pass; prettier reaches `prettier_variant_hugged.svelte`. This file carries
  the fixture's whole claim (see the second ⚠️ above).
- `prettier_variant_hugged.svelte` — prettier's stable form of that authoring.

## Related

- [body_blank_break](../body_blank_break_prettier_divergence/) — the rule this bounds
- [hoisted_boundary_sibling_kinds](../hoisted_boundary_sibling_kinds_prettier_divergence/) — the
  trim whose exclusions this enumerates
- [hoisted_boundary_convergence](../hoisted_boundary_convergence_prettier_divergence/) — the trim
  on a text neighbour
