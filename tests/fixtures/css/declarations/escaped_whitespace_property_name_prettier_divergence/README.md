# escaped_whitespace_property_name_prettier_divergence

A property name ending in an **escaped whitespace** — `a\ : x`. As in a value, `\ ` is a
valid escape whose escaped code point is that space (CSS Syntax 3 §4.3.4 / §4.3.7), so the
property is named `a ` and the space is part of the name, not padding before the colon.

tsv keeps it, so `input.svelte` formats to itself. Prettier trims the payload as padding and
strands the backslash onto the colon (`a\ : x` → `a\: x`), which escapes the colon: the
declaration now has no name→value separator, and prettier's output no longer parses
(tsv's CSS parser rejects it with `Expected '{', found ';'`). `output_prettier.svelte` pins it.

The hex-escape terminator before a colon (`\41 : red`), where both formatters keep the space,
is [escaped_property_name](../escaped_property_name/).

See [conformance_prettier_css.md §CSS: Values](../../../../../docs/conformance_prettier_css.md#css-values) ("Escaped whitespace ending a property name").
