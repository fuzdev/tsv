# inline_multiline_mixed_hug_prettier_divergence

An inline element with multiline mixed children (components, expressions, elements) lays out
**block-style** — both tags stay intact and the content sits on its own indented lines — even when
the author hugged the closing tag (no trailing whitespace before `</span>`). Prettier instead
dangles the closing delimiter (`{expr}</span⏎>`).

The authored boundary whitespace does not decide the layout: it is render-free under Svelte 5
(start/end-of-tag whitespace is removed at compile), so a hugged boundary and a whitespace boundary
are the same document. tsv therefore converges every authoring on one form, where prettier keeps two
— its hugged form dangles while its whitespace form stays intact (the last case here).

`prettier_variant_hug.svelte` is prettier's dangle form: prettier keeps it stable, tsv normalizes it
to the block-style input. `unformatted_ours_spaces.svelte` is a loose authoring tsv likewise
normalizes to input (prettier dangles it instead).

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
