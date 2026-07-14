# inline_multiline_leading_text_long_prettier_divergence

An inline `<span>` whose content ends with an element child (`<a>link</a>`) touching `</span>`, and
whose text is one character too long to fit.

The content goes multiline by **width**, and the layout question is what the closing boundary does.
The author left no whitespace there — but content-boundary whitespace is render-free under Svelte 5
(it is removed at compile), so its absence carries no more signal than its presence would. tsv lays
the content out **block-style**: both tags intact, content on its own indented line. Prettier reads
the hug as an instruction to keep the content glued to the closing tag, which forces it to **dangle**
the delimiter (`<a href="#">link</a></span⏎>`) — see `prettier_variant_dangle`, which prettier keeps
stable and tsv normalizes to `input`.

This is the width-driven counterpart of
[inline_boundary_whitespace_multiline](../inline_boundary_whitespace_multiline_prettier_divergence/)
(structural multiline). The 100-char case above it stays inline in both formatters, so the divergence
appears only once the content actually breaks.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
