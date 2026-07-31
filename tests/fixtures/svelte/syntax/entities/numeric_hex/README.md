# Numeric Hex Entities Test

Tests hexadecimal numeric character references in HTML/Svelte templates.

## Note on Uppercase X

The HTML5 spec's numeric-character-reference state opens a hex reference on either
`U+0078 x` or `U+0058 X`, so `&#X41;` decodes to `A`. Svelte's decoder spells only the
lowercase form and leaves `&#X41;` as literal text — a divergence, so the uppercase
cases live in
[spec_decoding_svelte_divergence](../spec_decoding_svelte_divergence/) rather than
here.
