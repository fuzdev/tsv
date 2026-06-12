# empty_value_prettier_divergence

Prettier preserves the source whitespace of an empty custom-property value verbatim; tsv normalizes it to a single space.

tsv: `--a: ;` (one canonical form)
Prettier: `--a:;`, `--a: ;`, `--a:     ;` all preserved as written

An empty value is the same value regardless of spacing — CSS Syntax 3 §"Consume a declaration" trims leading and trailing whitespace, so every variant parses to value `""`. The whitespace prettier preserves is not significant.

## Reason

`<declaration-value>?` makes a custom property's value optional (CSS Variables 1 §"Custom Property Value Syntax"). tsv normalizes the empty value to the single-space form CSS Variables 1 §"Serializing Custom Properties" mandates: "an empty custom property … must serialize with a single space as its value." Non-empty custom properties (`--normal: red;`) are unaffected; non-custom empty declarations (`color:;`) remain a parse error — a value is required there.

Catalogued under [CSS: Values](../../../../../../docs/conformance_prettier.md#css-values).

## Fixture Structure

- `input.svelte` — tsv canonical form (`--a: ;`)
- `prettier_variant_compact.svelte` — prettier preserves `--a:;` (no space); tsv normalizes to input
- `prettier_variant_spaces.svelte` — prettier preserves `--a:     ;` (extra spaces); tsv normalizes to input
