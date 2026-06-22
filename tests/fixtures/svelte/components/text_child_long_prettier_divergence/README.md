# text_child_long_prettier_divergence

A component (`<Comp>`) whose **body is long inline text**. When it overflows, the body lays out
**block-style** — both tags intact, content on its own indented line — exactly like an inline
element. The short companion (stays flush) is `components/text_child`.

Prettier instead **dangles** the tag delimiters (`<Comp⏎\t>…</Comp⏎>`); tsv keeps both tags intact.
Components are inline-classified, so this is the component counterpart to
`elements/inline_content_text_wrap`. The `unformatted_ours_compact` (single-line authoring) and
`prettier_variant_compact` (prettier's stable dangle) both normalize to `input.svelte` under tsv in
one pass.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
