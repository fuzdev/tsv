# multiline_value_inline_long_prettier_divergence

When an inline element has breaking attributes (multiline value) and long trailing text follows, Prettier allows the line to exceed printWidth (106 chars). tsv's fill breaks trailing words at the boundary.

tsv: wraps last word to stay within 100 chars
Prettier: keeps trailing text on one line (106 chars)

## Reason

**Print width.** Prettier's `handleTextChild` early-returns for the last text child without wrapping the previous inline element. When the element has breaking attrs, there's no break point between the closing tag and trailing text, so the line exceeds printWidth. tsv skips the wrapping too (matching Prettier), but the fill within the last text node still breaks at word boundaries.

See also [multiline_value_inline](../multiline_value_inline_prettier_divergence/) which shows both formatters match when trailing text is short. See [conformance_prettier.md §Svelte: Elements](../../../../../docs/conformance_prettier.md#svelte-elements) ("Fill after breaking attr").
