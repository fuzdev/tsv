# own_line_prettier_divergence

tsv: a `{#snippet}` takes its **own line**, exactly like a declaration tag (`{@const}` /
`{const …}` / `{let …}`), so every authoring of the fragment converges on one form. Prettier
keeps a stable form per authoring, welding the snippet to whatever the author wrote beside it.

## Reason

A snippet declares a binding and **renders nothing**, and `clean_nodes` **hoists it out of its
fragment before the whitespace rules run** (its `hoisted` list) — the same two facts that give a
declaration tag its own line. So the whitespace beside a snippet is not inter-sibling whitespace
at all: at a fragment edge the run is deleted, and in the interior the two runs it splits merge
back into the single whitespace rule 1 would have produced anyway. Breaking beside a snippet
therefore changes no render, and the layout question is free —

```
<div>text {#snippet fn()}x{/snippet}</div>   compiles with `text` trimmed   ← the run is DELETED
```

— so tsv answers it with the snippet's own line, which is where authors already put snippet
declarations and what keeps a declaration visually distinct from the content beside it. A
render-free run must not select a layout, the rule this whole section rests on
([§Svelte: Inline content block-style](../../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style)),
so the glued, spaced and newline authorings are one document and reach one form.

⚠️ **The one exception is a snippet GLUED to content on both sides**, where the break is *not*
render-free: `<div>a{#snippet fn5()}x{/snippet}b</div>` compiles to `ab`, while the own-line form
compiles to `a b` — a different document. That is the standing "a glued boundary is never split"
rule, and it is what bounds this one. Gluing is asked of the nearest **content** on each side: a
hoisted neighbour (another snippet, a declaration tag, `{@debug}`) is not content, so a run of
consecutive snippets is not glued to itself and each member still takes its line.

⚠️ **`<pre>` is unaffected** — inside a whitespace-preserving element boundary whitespace is
literal, so no line is free to take; pinned by
[elements/pre_block_await_key_snippet](../../../elements/pre_block_await_key_snippet/).

## Cases

- **lone snippet in a block** — `{#if cond}{#snippet fn1()}x{/snippet}{/if}`: the block goes
  multiline; the snippet has no sibling and still takes its line.
- **lone snippet in a component** — the same convergence in a component host.
- **text sibling** — `text1 {#snippet fn3()}x{/snippet}`: the snippet takes its line, the text
  keeps its own.
- **glued on one side** — `text2{#snippet fn4()}x{/snippet}`: the snippet hoists, so the text's
  run is a fragment-edge run whichever side of the snippet it sits on and the break is render-free.
- **glued on both sides** — `a{#snippet fn5()}x{/snippet}b` keeps the author's line, the exception
  above.
- **consecutive snippets** — `{#snippet fn6()}…{#snippet fn7()}…text3` breaks into three lines:
  each snippet's neighbour is hoisted, so neither is glued to content.
- **comment sibling** — `<!-- c -->{#snippet fn8()}…`: a comment IS content for the glue scan
  (its position is authorship, and it must stay render-safe under `preserveComments` too), but
  it glues only one side here, so the snippet still takes its line.
- **`{@debug}` neighbour** — `{@debug cond}{#snippet fn9()}…`: a hoisted non-declaration is not
  content, so the snippet is not glued; the `{@debug}` sits on its own line in the multiline
  layout (its weld target is the hoisted snippet, which is no target at all).
- **root fragment** — `{#snippet fn10()}x{/snippet}text4` at the top level: the root runs
  through a different printer path, so the trailing one-side glue is pinned there rather than
  assumed from the element hosts.

`prettier_variant_compact.svelte` writes every case on one line; prettier keeps it stable, tsv
normalizes it to `input`. (A spaced-boundary authoring converges too, but prettier's landing form
for it is per-host — a component/element boundary space trims to the compact form while a spaced
block boundary expands — so the spaced spellings are left to the authoring-independence audit
rather than pinned as a variant file.) The excluded glued shape is spelled identically in both
files — it has one form, not one per authoring.

## Related

- [tags/declaration_own_line](../../../tags/declaration_own_line_prettier_divergence/) — the same
  rule on the declaration tags, where it started
- [blocks/hoisted_boundary_convergence](../../hoisted_boundary_convergence_prettier_divergence/) —
  the hoist's other consequence, the edge trim, on the nodes no layout rule gives a line
  (`{@debug}`, `<title>`)

See
[conformance_prettier.md §Svelte: Inline content block-style](../../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
