# inline_separator_entity_collapse_prettier_divergence

tsv: an **entity-encoded** whitespace separator between two non-text siblings makes the content a
reflowable fill exactly as a literal space does, so both authorings converge on the one fully
inline form. Prettier keeps a distinct stable form per authoring.

## Reason

**Design choice — rule 1 of Svelte 5's whitespace model**, reached one spelling further out than
[inline_separator_tab](../inline_separator_tab_prettier_divergence/). An inter-sibling whitespace
run collapses to a single whitespace, so a separator's *presence* carries signal while its
*spelling* carries none — and an entity is a spelling. `&#9;` decodes to a tab, the compiler
collapses that run to one space, and the result is byte-identical to the literal-space twin's.
Letting the entity pick a different layout is a bug against rule 1 rather than a divergence from
prettier — prettier converges neither authoring, so it is no oracle here and the bar is
tsv-vs-tsv consistency. See
[§Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).

**The entity's own bytes are never rewritten.** tsv converges the *layout*, not the source: `&#9;`
stays `&#9;` in every form below, because respelling an entity is a content edit. That is the whole
reason the classification is split in two — the whitespace scalars read `raw` (so the node is
content and prints verbatim), while "is this separator interchangeable with a plain space?" reads
the **decoded** text. An `&nbsp;` decodes to a non-breaking space, which never collapses, so it
does *not* reach this rule and keeps its authored lines — pinned next door by
[inline_separator_nbsp_newline](../inline_separator_nbsp_newline/).

⚠️ For the **tag** pair the "renders identically" authority is the browser, not the compiler:
`clean_nodes` skips the collapse whenever a neighbor is an `ExpressionTag`, so `{a}&#9;{b}` reaches
compiled output with its entity intact where `<code>a</code>&#9;<code>b</code>` compiles to a single
space. Both are render-identical under `white-space: normal` — the model tsv's render-equivalence
oracle implements — so the convergence target is unchanged, but the tag case is not a rule-1
consequence and must not be restated as one.

## Cases

The three non-text sibling kinds — an expression-tag pair, an element pair and a component pair —
each in the converged inline form, plus the two other authorings of that same document:

- `variant_boundary_newline_entity.svelte` — the content boundaries newline-authored.
  **Dual-stable**: the authored air is the author's and both formatters preserve it (see
  [inline_boundary_air](../inline_boundary_air/)). What it pins here is that the entity
  separator does not pick a layout inside that preserved air either — the separator's spelling
  is inert on both sides of the boundary question.
- `prettier_variant_boundary_space_entity.svelte` — the boundaries space-authored, the third
  point of the hug ↔ space ↔ newline triangle. tsv normalizes it to `input` too (a render-free
  boundary run is trimmed whole); prettier holds a stable form per authoring, so one document
  reaches three prettier forms and one tsv form.

## Related

- [inline_separator_tab](../inline_separator_tab_prettier_divergence/) — the same rule for the
  literal tab spelling, with the newline held deliberately apart
- [inline_separator_entity_newline](../inline_separator_entity_newline/) — the complement: an
  entity separator in a run the author *did* break keeps its authored lines, since a run of pure
  siblings holds no prose to reflow
- [inline_separator_nbsp_newline](../inline_separator_nbsp_newline/) — the separator that looks
  like this one and is content
