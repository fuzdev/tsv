# hoisted_boundary_convergence_prettier_divergence

A **hoisted** node — `{@debug}`, `<title>` — is invisible to the whitespace rules, so the
whitespace between it and the fragment's edge is render-free and tsv trims it. Prettier keeps a
stable form per authoring.

## Reason

`clean_nodes` **hoists those node types out of the fragment before it trims** (its `hoisted`
list). Rule 2 — "whitespace at the start and end of a tag is removed completely" — then applies to
what is left, so the text beside a hoisted node *is* the fragment's first/last node and its edge
run is deleted:

```
{#if cond}text {@debug cond}{/if}   compiles to `text`   ← the run is DELETED
{#if cond}text <b>y</b>{/if}        compiles to `text <b>y</b>`   ← control, the run is kept
```

The two authorings of the first line are therefore one document, and a render-free run must not
select a layout — the rule this whole section rests on
([§Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style)).
Prettier holds a stable form for each authoring instead, so one document reaches two prettier
forms and one tsv form, exactly as at an ordinary content boundary
([blocks/content_boundary_convergence](../content_boundary_convergence_prettier_divergence/)).

⚠️ tsv's participating set is deliberately **narrower** than the oracle's, and every exclusion is
the same judgement: **a node that owns its own line keeps it.**

- A **declaration** (`{@const}` / `{const …}` / `{let …}` / `{#snippet}`) is given its own line by
  [tags/declaration_own_line](../../tags/declaration_own_line_prettier_divergence/) and
  [blocks/snippet/own_line](../snippet/own_line_prettier_divergence/) — the hoist's other
  consequence, read as a layout licence rather than as a trim. The break beside such a node is
  render-free for the same reason the trim is, so both are correct and only one can happen; the
  own line is the better form, and is where authors already put declarations.
- The `<svelte:head>` / `<svelte:window>` / `<svelte:body>` / `<svelte:document>` four are
  block-classified, so `handle_block_child` already gives each a line — and a fragment-edge line
  break is itself render-free, so trimming it and breaking it are both render-correct yet cannot
  both happen (`<svelte:body … />b` would trim to the glued form, whose next pass re-breaks it: an
  F1 2-cycle the fuzz gate caught).

What is left — `{@debug}` and `<title>` — is exactly the set no layout rule gives a line of its
own. `{@debug}` is not a declaration but a transient debugging aid, so welding it to its neighbour
keeps it out of the way of the code it inspects.

⚠️ **The hoist is not in Svelte 5's three published whitespace rules** — those state the collapse
and the edge trim but not *which nodes the edge is measured against*. The behavior is in
`clean_nodes` and is verified here against the compiler, not against the summary.

## Cases

- **trailing edge** — `text {@debug cond}`: the text is the last surviving node, so its trailing
  run goes.
- **leading edge** — `{@debug cond} text`: the mirror.
- **`<title>`** — `<svelte:head><title>text2</title> text3</svelte:head>`: the other participating
  hoisted kind (a `TitleElement` — `<title>` *inside* `<svelte:head>`; a bare `<title>` elsewhere
  is a RegularElement and not hoisted). Its trailing edge run trims the same way. One case only —
  a component allows a single `<svelte:head>` — and the interior rule is the same mechanism the
  `{@debug}` control below pins.
- **interior control** — `a {@debug cond} b`. With content on **both** sides the hoist does not
  make either run an edge: the two runs merge into a single space (`a b`), so one space survives
  and gluing would be a different document (`ab`). This is what bounds the rule — it reaches the
  fragment's edges, never a run between two content nodes.
- **root fragment** — `text1{@debug cond}` at the end of the file. The root is a fragment too, and
  it runs through a different printer path, so it is pinned rather than assumed. Its position is
  load-bearing: earlier in this same file the identical pair is *interior* and keeps its space.

`prettier_variant_spaced.svelte` carries the spaced authoring of every edge case (prettier keeps
it; tsv normalizes it to `input`) with the interior control unchanged, so the file isolates the
edge/interior split on its own.

`divergent_variant_boundary_newline.svelte` is the newline authoring of the first case. It shows
the rule firing in the **multiline** arm as well: tsv collapses the hoisted run
(`text{@debug cond}`) but leaves the block body on its own line, so it lands on a *third* stable
form rather than on `input`. That residual is the ordinary block-body behavior, not this rule — a
newline-authored body keeps its lines with a plain text body too (`{#if cond}⏎\ttext⏎{/if}` is its
own fixed point).

## Related

- [tags/declaration_own_line](../../tags/declaration_own_line_prettier_divergence/) — the hoist's
  other consequence, on the tags this rule no longer covers
- [blocks/content_boundary_convergence](../content_boundary_convergence_prettier_divergence/) —
  the same convergence at an ordinary block-body boundary
- [elements/inline_boundary_whitespace](../../elements/inline_boundary_whitespace_prettier_divergence/) —
  and at an element's content boundary
