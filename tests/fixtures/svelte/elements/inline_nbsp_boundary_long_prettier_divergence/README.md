# inline_nbsp_boundary_long_prettier_divergence

Pins how a non-breaking space interacts with the break-before rule. An `<code>` preceded by
same-line text overflows the line (a trailing `tex` tips it past printWidth), so tsv breaks at
the whitespace boundary before `<code>` and the element moves to a **fresh line**, collapsing
inline (`<code>.x</code>`). The NBSP (literal U+00A0 or `&nbsp;`) is content, not collapsible
whitespace, so it keeps `tex` glued to `</code>` on that line — it is never broken at.

The 100-char cases stay inline (control); the 101-char cases break before `<code>`. tsv breaks
at the whitespace boundary before the element rather than dangling its opening tag on the text
line; prettier keeps the opening tag on the text line and dangles it. The `unformatted_ours_*`
variant is a compact authoring tsv normalizes to `input.svelte`; `prettier_variant_*` pins
prettier's stable dangle form (which tsv also normalizes to input).

The boundary before `<code>` is inter-node whitespace (render-free under Svelte 5); the NBSP is
render-significant content and is never split — so the break is render-equivalent.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
