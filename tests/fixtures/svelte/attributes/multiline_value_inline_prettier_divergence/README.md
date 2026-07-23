# multiline_value_inline_prettier_divergence

An inline `<span>` preceded by same-line text, whose attribute value contains a literal
newline (`data-attr="a,⏎  b"`) that forces the opening tag to wrap. tsv breaks at the
whitespace boundary before the `<span>` so it starts a **fresh line** rather than dangling
its opening tag at the end of the `text` line. This is the break-before rule triggered by a
forced-multiline *attribute* rather than by width.

tsv: `text` on its own line, `<span` starts the next line and wraps its attributes.
Prettier: keeps `text <span` on one line and dangles the attributes there. The
`unformatted_ours_*` variant is a compact authoring tsv normalizes to `input.svelte`;
`prettier_variant_*` pins prettier's stable dangle form (which tsv also normalizes to input).

The boundary before the `<span>` is inter-node whitespace (render-free under Svelte 5), so the
break is render-equivalent.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
