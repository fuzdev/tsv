# union_prettier_ignore_trailing_head_prettier_divergence

A directive **trailing the type alias head** — after the `=`, at the end of the head's
line, with the value on the next line:

```ts
type A = // prettier-ignore
	{ x: 1 } | b;
```

tsv's placement rule is total and placement-only: a directive with **any content before it
on its physical line** that is not glued directly before the value is **trailing → inert**.
tsv keeps the comment where the author put it (the standard head-trailing comment layout)
and reformats the value normally.

Prettier instead attaches the comment leading to the value, relocates it to its **own
line**, and freezes the **whole value** verbatim (`output_prettier.svelte`).
`unformatted_ours_perturbed.svelte` carries an unformatted value interior (`{x:1}`): tsv
normalizes it to input (inert); prettier keeps it frozen.

`variant_frozen.svelte` (prettier's output on the perturbed form) is **dual-stable**:
prettier's relocation puts the directive **own-line before the value**, and that is a
placement tsv honors (Rule A first-member freeze) — so tsv keeps the relocated frozen
form too. The divergence is only in what each tool does to the *head-trailing* authoring;
once relocated own-line, the two agree.

The annotation-head analog (`let v: // prettier-ignore`) is the sibling
[union_prettier_ignore_trailing_annotation_head_prettier_divergence](../union_prettier_ignore_trailing_annotation_head_prettier_divergence/),
where prettier freezes **without** relocating, so its frozen form stays one tsv
normalizes.

## Reason

One simple, teachable rule — tsv honors a directive only **own-line before a member**
(freezes that member) or **glued directly before a value/member** (freezes it whole);
everything else is inert. A head-trailing directive is neither, and honoring it would
require the same backward/sideways binding ambiguity the trailing-member case
(`union_prettier_ignore_trailing_inert`) already refuses.

See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
