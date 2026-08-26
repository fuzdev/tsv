# hoisted_boundary_sibling_kinds_prettier_divergence

The hoisted-edge trim reads the **edge**, not whether the neighbour happens to be a text.
`clean_nodes` lifts a `{@debug}` out of its fragment before it trims, so whatever sibling stands
beside it *is* the fragment's first or last node and the run between them is a render-free edge
run — for an inline element, a component and every tag kind exactly as for the text sibling that
[hoisted_boundary_convergence](../hoisted_boundary_convergence_prettier_divergence/) already pins.
Every authoring of those reaches the one glued form. Prettier keeps a stable form per authoring.

## Reason

Same mechanism as the base fixture, asked of the other axis. The base fixture's cases all put a
**text** node beside the hoisted node, and its own wording ("the text beside a hoisted node *is*
the fragment's first/last node") is that reading; the sibling's kind never entered it. `clean_nodes`
does not consult the kind either — it deletes the fragment's edge runs after the hoist, whatever
stands at the edge — which is the same answer tsv gives at an **element's own** content edge, where
the boundary run trims for a text, element, component, tag, comment and void child alike
([elements/inline_boundary_whitespace](../../elements/inline_boundary_whitespace_prettier_divergence/),
[elements/title_boundary_whitespace](../../elements/title_boundary_whitespace_prettier_divergence/)).
Every authoring here compiles byte-identically (`render_compare`: identical), so a run that selects
a layout is a render-free run selecting one — the rule this section rests on.

⚠️ **Being deletable licenses the trim; it does not decide it.** Some form has to be chosen, and
the base rule's own exclusion picks which: **a node that owns its own line keeps it** — asked here
of *both* ends of the run, which is what bounds this fixture's claim.

- The **hoisted** end is a `{@debug}`, the one hoisted kind with no layout claim anywhere (a
  transient debugging aid, welded out of the way of the code it inspects). A hoisted `<title>` is
  an **element**, and among sibling elements it owns a line like any other — welding
  `<svelte:head><title>t</title><meta … /></svelte:head>` would destroy structure the author
  expressed and that no other spelling could restore, so it is a control here. Its edge run beside
  a **text** still trims; that path is the content handler's and the base fixture pins it.
- The **content** end must be **content at all**, and a sibling whose own authored newline flows
  (`sibling_newline_flows`): a text, an inline element, a component or a tag. A comment, a
  `<br />`, a control-flow block and a block element own their line, so the run is that line's
  separator rather than a weldable edge — and trimming it there would take away the author's only
  lever, since every spelling of the run, blank included, collapses to the same weld.

  ⚠️ Those are **two** tests on one side, not one. The flow test is asked of neighbour *kinds* and
  knows nothing about hoisting, and the two part on a `<title>`: it is a `SpecialElement` that is
  not block-classified, so it FLOWS. Asking flow alone welded
  `<svelte:head><title>t</title> {@debug expr}</svelte:head>` — a run between two nodes that are
  *both* hoisted, so the fragment has no content end at all — while the same `<title>` beside an
  ordinary element sibling kept its line by the bullet above. The `{#if}` pair below is that
  discriminator, varying one node's kind and nothing else.

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).

## Cases

- **trailing edge, every flowing sibling kind** — an inline element, a component, and all three
  tags (`{expr}`, `{@html}`, `{@render}`) before `{@debug}`.
- **leading edge** — the mirror, before an element and before a tag.
- **block body** — the same edge inside an `{#if}` and an `{#each}` body, where a blank is
  otherwise a Tier-2 signal that forces the body open. The run is deleted, so the blank spelling
  cannot force it: the body stays hugged from all three authorings. This is the guard on the rule
  that *every* reader taking a signal from this run's bytes has to ask the same question — a
  fragment-level blank gate included.
- **the text sibling** — the case the base fixture pins, carried here as the parity control that
  keeps "the kind does not decide" from being satisfied by a rule that trims nothing.
- **interior control** — `<span>inline1</span> {@debug expr} <span>inline2</span>`. With content on
  both sides the hoist makes neither run an edge: the two merge into one rendered space, so the
  space survives and gluing would be a different document.
- **line-owning sibling controls** — a comment, a `<br />`, a control-flow block and a block
  element, each keeping the run.
- **`<title>` controls**, all in the one `<svelte:head>` a component may have, with the nested
  bodies first so `<title>text2</title>` keeps the trailing edge its own case needs:
  - as the **hoisted** end — the element beside an element sibling, keeping its line;
  - as the **content** end — `{#if cond}<title>text3</title> {@debug expr}{/if}`, where both ends
    are hoisted so there is no content end at all and the run is kept. `<title>` hoists here
    because head context carries through a nested block, and it *flows*, so this is the case the
    flow test alone gets wrong;
  - its **`<b>` twin** — `{#if cond}<b>text3</b>{@debug expr}{/if}`, the same shape varying only
    that node's kind. An inline element is content, so it IS the content end and the run welds.
    Without it the `<title>` case is satisfied by a rule that welds nothing in that position.
- **no-content control** — two `{@debug}` tags with a blank line between them and nothing else in
  the fragment. Neither side of the run is content, so nothing is weldable and the blank survives;
  without it the rule would collapse a file of debug tags onto one line. It is the same class as
  the `<title>`-as-content-end case above, spelled with a hoisted kind that does not flow either
  way — which is why that one and not this one is what pins the hoist test.

## Files

- `prettier_variant_spaced.svelte` — the space authoring, which prettier keeps stable and tsv
  normalizes to `input`.
- `unformatted_ours_newline.svelte` / `unformatted_ours_blank.svelte` — the newline and blank-line
  authorings, with each container's own boundaries left glued so the only run in play is the one at
  the hoisted edge. tsv reaches `input` from both, the blank included: a deleted run has no
  boundary left to carry a Tier-2 signal, the same answer the base fixture gives.
- `divergent_variant_newline.svelte` / `divergent_variant_blank.svelte` — prettier's own stable
  forms of those two, which tsv rewrites to a *third* form: the hoisted run trims, but the
  containers' authored boundary air keeps them block-style. That residual is the ordinary
  boundary-air behavior, not this rule.

## Related

- [hoisted_boundary_convergence](../hoisted_boundary_convergence_prettier_divergence/) — the base
  rule, on a text sibling
- [tags/declaration_own_line](../../tags/declaration_own_line_prettier_divergence/) — the hoist's
  other consequence, on the node kinds that take a line instead of a trim
- [elements/inline_sibling_space_before_bounding](../../elements/inline_sibling_space_before_bounding_prettier_divergence/) —
  the space before a run-ending sibling, whose `{@debug}` cells this rule reaches
