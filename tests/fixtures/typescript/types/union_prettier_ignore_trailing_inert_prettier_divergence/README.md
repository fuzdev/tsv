# union_prettier_ignore_trailing_inert_prettier_divergence

A **trailing** `// prettier-ignore` on a union member (`| a1&a2 // prettier-ignore`).
tsv honors an ignore directive only where it **precedes** the node it targets, so a
trailing directive is inert: both members reformat normally.

`input.svelte` is the canonical form both formatters keep (dual-stable):

```ts
type U =
	| (a1 & a2) // prettier-ignore
	| (b1 & b2);
```

`prettier_variant_frozen.svelte` is prettier's stable form, where the trailing
directive freezes the **preceding** member backward (`| (a1&a2) // prettier-ignore`).
tsv normalizes that form to `input.svelte` (it does not honor the trailing
directive), so it is a `prettier_variant_*` divergence — prettier keeps it, tsv
reformats it to input.

This fixture passes today (tsv is already inert to a trailing directive) and pins
that behavior as permanent: it is the regression control that, when the honored
*leading* positions are implemented, a trailing directive must **not** start freezing
its preceding member, and must never freeze the **following** member (the wrong-node
misbind class).

## Reason

Honoring a directive only in the position that unambiguously precedes its target
keeps the binding local and predictable; trailing usage does not appear in real
corpora, only in prettier's own test suite. A trailing directive that reformats
normally cannot silently freeze or misbind an adjacent member.

See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
