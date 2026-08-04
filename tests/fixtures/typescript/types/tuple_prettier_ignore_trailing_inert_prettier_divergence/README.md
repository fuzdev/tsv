# tuple_prettier_ignore_trailing_inert_prettier_divergence

A **trailing** `// prettier-ignore` on a tuple member (`{ x: 1 }, // prettier-ignore`).
tsv honors an ignore directive only where it **precedes** the node it targets, so a
trailing directive is inert: both members reformat normally.

`input.svelte` is the canonical form both formatters keep (dual-stable):

```ts
type T = [
	{ x: 1 }, // prettier-ignore
	{ y: 2 }
];
```

`prettier_variant_frozen.svelte` is prettier's stable form, where the trailing
directive freezes the **preceding** member backward (`{ x:   1 }, // prettier-ignore`).
tsv normalizes that form to `input.svelte` (it does not honor the trailing
directive), so it is a `prettier_variant_*` divergence — prettier keeps it, tsv
reformats it to input.

The tuple-family control for the same permanent trailing-inert rule the union
fixture pins (`union_prettier_ignore_trailing_inert_prettier_divergence`): a
trailing directive must not start freezing its preceding member, and must never
freeze the **following** member (the wrong-node misbind class). Both members carry
perturbable object interiors (`{ x:   1 }` / `{ y:   2 }` in
`unformatted_ours_perturbed`) so a misbound freeze in either direction leaves
visible unformatted bytes.

## Reason

Honoring a directive only in the position that unambiguously precedes its target
keeps the binding local and predictable; trailing usage does not appear in real
corpora, only in prettier's own test suite. The rule is placement-only and uniform
across every member list (Rule A), so the tuple family behaves exactly like the
union/intersection families.

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
