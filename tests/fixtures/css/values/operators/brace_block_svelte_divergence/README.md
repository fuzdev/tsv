# brace_block_svelte_divergence

A `{…}` block **inside** a custom property's value — `--a: a{1.5}c(2.5)`. Svelte's
`parseCss` rejects the declaration (`css_expected_identifier`); tsv accepts it and
formats it as prettier does.

See [conformance_svelte.md §CSS Corrections](../../../../../../docs/conformance_svelte.md#css-corrections)
— entry "Brace block inside a custom property's value".

## Why tsv accepts

css-syntax-3 §"Consume a declaration": a custom property's value is the original source
text of its tokens, whatever they are; only a **non**-custom property refuses a top-level
`{}`-block that stands beside any other value. So `--a: a{1.5}c(2.5)` is a valid
declaration and `b: a{1.5}c(2.5)` is not (both parsers reject the latter).

## How it formats

The block's braces are members of the value like prettier's value parser reads them
(`{` and `}` are token kinds of their own there), so each brace is glued to the block's
interior always and to the members outside it only as authored, and the words between
the braces normalize: `a{1.50}c(2.50)` → `a{1.5}c(2.5)`, `a {1.50} c(2.50)` →
`a {1.5} c(2.5)`. An operator beside a brace reads the brace as a word — glued to it as
authored, spaced from its operand (`a{1.5}/ 2.5`, `a *{1.5}c`). Prettier prints every
declaration here identically, so the fixture carries plain `unformatted_*` variants.

Contrast a `[…]` block, whose interior is word text to both formatters and is kept as
written (`a[1.50]c(2.5)`), and the **whole-value** block
([block_value](../../variables/block_value_svelte_prettier_divergence/)), the one
`{}`-block css-syntax-3 singles out, which tsv keeps as a single-line opaque value.
