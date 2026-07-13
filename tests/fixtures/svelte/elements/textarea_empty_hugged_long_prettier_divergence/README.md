# textarea_empty_hugged_long_prettier_divergence

An **empty** `<textarea>` hugged by an inline `<label>`, at the print-width boundary.

`<textarea>` is whitespace-sensitive, but it has no content here — so nothing at the *label's*
boundary is whitespace-significant, and the `<label>` lays out block-style like any other inline
element: both tags intact, the textarea on its own indented line. (The `<textarea>`'s own mandatory
`>` placement is untouched; see [textarea_content_long](../textarea_content_long/) and
[textarea_attrs_long](../textarea_attrs_long/), where the dangle is real and stays.)

Prettier instead hugs the label's content and **dangles** its delimiter (`</textarea></label⏎>`).
That suffix is not free: it adds seven columns to the textarea's line, which pushes even the
100-char case over print width and forces the textarea's attributes to wrap. tsv, having no suffix
to carry, keeps the 100-char case inline and wraps only at 101 — so the fixture's two cases still
straddle a real boundary, just tsv's.

`prettier_variant_label_dangle` is prettier's stable form (tsv normalizes it to `input`).

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
