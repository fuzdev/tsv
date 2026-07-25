# union_prettier_ignore_trailing_inert_prettier_divergence

A **trailing** `// prettier-ignore` on a union member (`| ({ x: 1 } & a2) // prettier-ignore`).
tsv honors an ignore directive only where it **precedes** the node it targets, so a
trailing directive is inert: both members reformat normally.

`input.svelte` is the canonical form both formatters keep (dual-stable):

```ts
type U =
	| ({ x: 1 } & a2) // prettier-ignore
	| ({ y: 2 } & b2);
```

`prettier_variant_frozen.svelte` is prettier's stable form, where the trailing
directive freezes the **preceding** member backward (`| ({x:1}&a2) // prettier-ignore`,
the paren re-synthesized outside the frozen slice).
tsv normalizes that form to `input.svelte` (it does not honor the trailing
directive), so it is a `prettier_variant_*` divergence — prettier keeps it, tsv
reformats it to input.

This fixture pins the inert behavior as permanent: it is the regression control
that a trailing directive must **not** start freezing its preceding member, and
must never freeze the **following** member (the wrong-node misbind class). Both
members carry an **object interior** (`{x:1}` / `{y:2}` in `unformatted_ours_perturbed`)
so a misbound freeze in either direction leaves visible unformatted bytes — a
composite following member whose first sub-member has no perturbable interior
would let a following-member freeze pass invisibly.

## Reason

Honoring a directive only in the position that unambiguously precedes its target
keeps the binding local and predictable; trailing usage does not appear in real
corpora, only in prettier's own test suite. A trailing directive that reformats
normally cannot silently freeze or misbind an adjacent member.

See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
