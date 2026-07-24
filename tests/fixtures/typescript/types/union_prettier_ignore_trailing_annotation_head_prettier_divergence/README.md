# union_prettier_ignore_trailing_annotation_head_prettier_divergence

A directive **trailing a type annotation's head** — after the `:`, at the end of the
head's line, with the annotation on the next line:

```ts
let v: // prettier-ignore
	{ y: 2 } | c1;
```

tsv's placement rule is total and placement-only: a directive with **any content before it
on its physical line** that is not glued directly before the value is **trailing → inert**.
tsv keeps the comment where the author put it (the standard head-trailing comment layout,
continuation-indented) and reformats the annotation normally.

Prettier also keeps the directive trailing the `:` but **freezes the annotation** and
drops it to the head's indent level (`output_prettier.svelte`).
`unformatted_ours_perturbed.svelte` carries an unformatted interior (`{y:2}`): tsv
normalizes it to input (inert); prettier keeps it frozen
(`prettier_variant_frozen.svelte`, prettier-stable, tsv normalizes it to input).

Unlike the alias-head sibling
[union_prettier_ignore_trailing_head_prettier_divergence](../union_prettier_ignore_trailing_head_prettier_divergence/),
prettier does **not** relocate this directive own-line — its frozen form keeps the
head-trailing placement, which tsv is inert to, so tsv normalizes prettier's form.

## Reason

One simple, teachable rule — tsv honors a directive only **own-line before a member**
(freezes that member) or **glued directly before a value/member** (freezes it whole);
everything else is inert. A head-trailing directive is neither, and honoring it would
require the same backward/sideways binding ambiguity the trailing-member case
(`union_prettier_ignore_trailing_inert`) already refuses.

See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
