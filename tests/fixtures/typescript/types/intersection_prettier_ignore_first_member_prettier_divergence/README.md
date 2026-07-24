# intersection_prettier_ignore_first_member_prettier_divergence

An own-line `// prettier-ignore` before an intersection type freezes **only the first
member** — the same Rule A that governs every honored list position (a directive between
`{` and the first class member freezes that member, not the whole body). The separators
(` & `) are parent-owned and the later members reformat normally.

**tsv** keeps `input.svelte` stable:

- `type A` — `{a:1}` is frozen (its missing interior spaces survive), `{ b: 2 }` and
  `{ c: 3 }` reformat;
- `type B` — freezing `a1` is a visual no-op, `a2 & c1` reformat;
- `type C` — a leading `&` on a multi-member intersection normalizes away (the first
  member is `a2`'s sibling, not a one-element wrapper).

**Prettier** freezes the **whole** intersection verbatim — it has no intersection printer,
so its `prettier-ignore` handling is an unmaintained emergent passthrough. It keeps
`input.svelte` stable too, *and* keeps `prettier_variant_frozen.svelte` stable — the
fully-frozen form (`{a:1}  &  {b:2} & {c:3}`, `a1&a2&c1`, `& a1&a2`) that **tsv normalizes
to `input.svelte`**. That variant is the divergence: prettier holds it; tsv reformats the
non-first members.

## Reason

Freezing only the first member keeps the directive's target consistent with every other
honored list position in tsv, rather than special-casing intersections to whole-node
freezing. Prettier's whole-node behavior is an accident of it lacking an intersection
printer — and it is not even a stable contract there: the adjacent
`intersection_prettier_ignore_between_members_prettier_divergence` fixture shows prettier
losing the freeze entirely at its own fixed point (◆prettier_bug).

See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
