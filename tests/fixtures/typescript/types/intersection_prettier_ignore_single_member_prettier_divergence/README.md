# intersection_prettier_ignore_single_member_prettier_divergence

A frozen **single-member** intersection — the mirror of
[union_prettier_ignore_single_member](../union_prettier_ignore_single_member_prettier_divergence/).
Reformatting collapses a 1-element intersection (drops its `&`), so freezing only the
member is non-idempotent — pass 2 sees a bare member no longer routed through the
intersection and reformats it. tsv resolves this by the sole member's shape:

- **leaf / object** (`& {a:1}`, `& foo`) — freeze the WHOLE intersection span verbatim,
  keeping the `&`. That is idempotent (pass 2 re-recognizes the single-member
  intersection). Prettier instead drops the `&` and freezes the bare member (`{a:1}`,
  `foo`), recorded in `output_prettier.svelte`.
- **composite** (`a1 | a2`) — the 1-element intersection is transparent: tsv collapses it
  and lets the inner Union/Intersection apply Rule A inside (freezing the union's first
  member). The tsv-stable form is the bare `a1 | a2`; prettier keeps `a1 | a2` too, so
  `type C` shows no output divergence — it documents the transparency that keeps the
  leaf/object arm from whole-freezing every single-member intersection.

## Reason

Keeping the `&` on a frozen leaf/object single-member intersection is the only way to
honor the directive idempotently without a bare-type freeze mechanism; the small
divergence (a kept `&` prettier drops) is preferable to silently dropping the directive
or a non-idempotent member-only freeze. Identical to the union sibling's rationale.
See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
