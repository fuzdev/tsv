# inline_welded_run_travel_long_prettier_divergence

The travel half of the glued rule, generalized past the shape that first exposed it
([inline_fold_glued_head_long](../inline_fold_glued_head_long_prettier_divergence/)). A welded
run — nodes byte-glued to each other, so no break may land anywhere inside it — still has the
whitespace boundary in *front* of it, which is inter-node whitespace the compiler collapses to
one space. tsv spends that one and the whole run travels to a fresh line **together**.

The rule belongs to the **unit**, not to whatever happens to end it. These cases vary what ends
the run, what the run is made of, and what follows it, and all reach the same form:

- **no terminal tail, line @100** — the run still fits after its preceding word (control).
- **no terminal tail, @101** — the run travels. Nothing follows the last element, so there is no
  after-element fold here at all; the unit is measured and moved on its own account.
- **non-terminal follower** — the same run followed by mid text and a further element. The run
  travels exactly as it does with nothing after it, and the follower packs after it on the fresh
  line. The sibling contrasts are the assertion: an element follower and a terminal tail both
  travel too, so the decision cannot depend on what follows the unit.
- **three elements, middle one overflows** — the element that no longer fits is *mid-run*, and
  the whole run still travels as one. No earlier short element can strand a later wide one.
- **three elements, last one overflows** — the crossing point sits past *every* earlier member:
  each fits where it stands and only the last element is out of width, yet the whole run still
  travels as one. Together with the middle-overflow case this pins the no-stranding claim from
  both sides of the run.
- **`&nbsp;` glue** — the glue is content rather than an absent character: an NBSP renders as
  itself and never collapses, so the run is unbreakable for a different reason and the break must
  never land on it.
- **component pair** and **expression tag** — the run's members are not all HTML elements.
- **tag-headed run, @100 / @101** — the run's HEAD is an expression tag (`{expr}.w<b>…</b>`).
  At exactly 100 the run packs onto the text line; at 101 it travels whole. The crossing point
  sits past the tag, in the last element — the boundary's measurement must reach *through* the
  tag to see it.
- **tag-headed run, the tag is the crossing point** — the same run where the width runs out
  inside the tag itself rather than past it. Both sides of the tag reach the same travel form:
  which member crosses the width cannot matter (the sibling of the mid-run/last-element pair,
  on the tag axis).
- **run welded through a mid-run tag** — element-tag-element glue
  (`<code>.x</code>{ex}<b>…</b>`), crossing past the tag: the walk from the boundary in front
  must pass through the tag to reach the element that no longer fits.

tsv: the run travels intact. Prettier keeps it on the text line and dangles the tag delimiters
(double-dangling `<b⏎>yy</b⏎>` where both ends must give) — see `prettier_variant_dangle.svelte`,
prettier's stable form, which tsv normalizes to `input.svelte`.
`unformatted_ours_compact.svelte` is the compact one-line authoring both formatters start from:
tsv normalizes it to `input.svelte`, prettier to the dangle form. `input.svelte` is itself
prettier-stable, so there is no `output_prettier.svelte`.

Every boundary tsv moves is render-free — the one it spends is inter-node whitespace, and the
glued boundaries inside the run are never touched — so the output renders identically to the
input (confirmed by `render_compare`).

⚠️ A regression that split one of the interior boundaries would be invisible to every
idempotency-shaped gate: the split form is its own fixed point, so F1, the seeded fuzzer,
`authoring_audit` and the round-trip all pass through it. Only the render oracle sees it, and
only on the `real` corpus tier.

## Reason

Design choice: "unbreakable inside" is not "immovable" — the unit spends the render-free boundary
in front of it rather than standing and overrunning, and tsv lays wrapping inline content out
block-style where prettier dangles the tag delimiters.
See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
