# union_prettier_ignore_single_member_prettier_divergence

A frozen **single-member** union. Reformatting collapses a 1-element union (drops its `|`),
so freezing only the member is non-idempotent — pass 2 sees a bare member no longer routed
through the union and reformats it. tsv resolves this by the sole member's shape:

- **leaf / object** (`| {a:1}`, `| foo`) — freeze the WHOLE union span verbatim, keeping the
  `|`. That is idempotent (pass 2 re-recognizes the single-member union). Prettier instead
  drops the `|` and freezes the bare member (`{a:1}`, `foo`), recorded in
  `output_prettier.svelte`.
- **composite** (`a1 & a2`) — the 1-element union is transparent: tsv collapses it and lets
  the inner Union/Intersection apply Rule A inside (freezing the intersection's first member).
  The tsv-stable form is the bare `a1 & a2` (identical to the
  `intersection_prettier_ignore_first_member` behavior); prettier keeps `a1 & a2` too, so
  `type C` shows no output divergence — it documents the transparency that keeps the
  leaf/object arm from whole-freezing every single-member union.

## Reason

Keeping the `|` on a frozen leaf/object single-member union is the only way to honor the
directive idempotently without a bare-type freeze mechanism; the small divergence (a kept
`|` prettier drops) is preferable to silently dropping the directive or a non-idempotent
member-only freeze. See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
