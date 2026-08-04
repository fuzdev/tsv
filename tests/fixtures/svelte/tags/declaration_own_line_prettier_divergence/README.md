# declaration_own_line_prettier_divergence

tsv: a **declaration tag** — `{@const}`, `{const …}`, `{let …}` — takes its **own line**, so every
authoring of the fragment converges on one form. Prettier keeps a stable form per authoring, welding
the tag to whatever the author wrote beside it.

## Reason

A declaration tag declares a binding and **renders nothing**, and `clean_nodes` **hoists it out of
its fragment before the whitespace rules run** (its `hoisted` list). So the whitespace beside it is
not inter-sibling whitespace at all: at a fragment edge the run is deleted, and in the interior the
two runs it splits merge back into the single whitespace rule 1 would have produced anyway. Breaking
beside such a tag therefore changes no render, and the layout question is free —

```
{#if cond}text {@const x = 1}{/if}   compiles to `text`   ← the run is DELETED
{#if cond}a {@const x = 1} b{/if}    compiles to `a b`    ← the two runs MERGE
```

— so tsv answers it with the tag's own line, which is where authors already put these tags (41 of 41
declaration tags across the corpus sit on their own line) and what makes a run of them read as the
declarations they are. A render-free run must not select a layout, the rule this whole section rests
on ([§Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style)),
so the glued, spaced and newline authorings are one document and reach one form.

⚠️ **The one exception is a tag GLUED on both sides**, where the break is *not* render-free:
`{#if c}a{@const x = 1}b{/if}` compiles to `ab`, while the own-line form compiles to `a b` — a
different document. That is the standing "a glued boundary is never split" rule, and it is what
bounds this one. Gluing is asked of the nearest **content** on each side: a hoisted neighbour is not
content, so a run of consecutive tags is not glued to itself and each member still takes its line.

⚠️ **`{@debug}` is deliberately excluded.** It is not a declaration — it is a transient debugging
aid, and welding it to its neighbour keeps it out of the way of the code it inspects. Its edge run
still TRIMS instead, the hoist's other consequence —
[blocks/hoisted_boundary_convergence](../../blocks/hoisted_boundary_convergence_prettier_divergence/).

⚠️ **The hoist is not in Svelte 5's three published whitespace rules** — those state the collapse
and the edge trim but not *which nodes the edge is measured against*. The behavior is in
`clean_nodes` and is verified here against the compiler, not against the summary: every form in this
directory compiles byte-identically.

## Cases

- **leading edge** — `{@const} text1`: the tag is the fragment's first node.
- **trailing edge** — `text2 {@const}`: the mirror.
- **interior** — `a {@const} b`. Content on both sides, so the two runs merge to one space rather
  than vanishing; the space survives the break, so the tag takes its line here too.
- **glued both sides** — `a{@const x = 1}b` keeps the author's line, the exception above.
- **glued through a hoisted run** — `a{@const x = 1}{@const y = 2}b`: the glue scan steps OVER a
  hoisted neighbour, so both tags are glued to content and the whole run keeps the author's line
  (hoisting deletes the tags before the trim, so the split form would render `a b` where this
  renders `ab`).
- **non-collapsible edge** — `a&nbsp;{@const x = 1}b`: an NBSP edge is content, not a separator,
  so it glues — a wider whitespace class here would misread it and split a glued boundary
  (`a\u{a0}b` → `a\u{a0} b`, a render change). A form feed edge glues the same way.
- **lone tag** — `{#if cond}⏎\t{@const x = 1}⏎{/if}`: the tag touches only the block's boundaries,
  which are not content, so it is not glued and still takes its line — the block goes multiline.
- **consecutive tags** — `{@const x = 1}{@const y = 2}text3` breaks into three lines: with the
  fragment edge on the far side of the run, neither tag reaches content on BOTH sides (contrast
  the glued-through-run case above, where content closes the run at each end).
- **element content** — `{const}` / `{let}` carry no placement restriction (unlike `{@const}`, which
  must be a block/component child), so the rule reaches an element's fragment as well.
- **`{@debug}` control** — `text6{@debug cond}` stays welded, pinning the exclusion.

`prettier_variant_compact.svelte` writes every case on one line and
`prettier_variant_spaced.svelte` spells the same boundaries with spaces; prettier keeps both stable,
tsv normalizes both to `input`. The one exception is the lone tag's spaced spelling
(`{#if cond} {@const x = 1} {/if}`), which prettier itself expands to the own-line form here — so
the spaced file spells that case as `input` does. The glued-both-sides exception is spelled
identically in all three files — it has one form, not one per authoring. The `{@debug}` control is
the one excluded shape the variant files respell: both spell it `text6 {@debug cond}`, which tsv
converges to the welded `input` form through the hoist's *trim*
([blocks/hoisted_boundary_convergence](../../blocks/hoisted_boundary_convergence_prettier_divergence/))
— so the control pins both halves at once: no own line, and the trim that takes its place.

## Related

- [blocks/snippet/own_line](../../blocks/snippet/own_line_prettier_divergence/) — the same rule on
  `{#snippet}`, which declares a binding and hoists alike
- [blocks/hoisted_boundary_convergence](../../blocks/hoisted_boundary_convergence_prettier_divergence/) —
  the hoist's other consequence, the edge trim, on the nodes this rule does not cover
- [blocks/content_boundary_convergence](../../blocks/content_boundary_convergence_prettier_divergence/) —
  the same convergence at an ordinary block-body boundary
- [tags/const/const_contexts](../const/const_contexts/) and
  [tags/declaration/declaration_contexts](../declaration/declaration_contexts/) — the own-line form
  in every host, where both formatters already agree

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
