# inline_element_wide_multiattr_long_prettier_divergence

The HTML-element counterpart of `inline_component_wide_multiattr_long`: a wide inline element with
several attributes (each of which *could* break onto its own line) is too wide to share a line with
the preceding text. tsv drops the whole element to its own line, where all its attributes still fit
on one line, keeps the preceding word (`and`) hugged, and puts the trailing text (`tail1 tail2
tail3`) on its own line — a wide inline child that drops owns its line. Prettier instead hugs
`and <a` onto the text line and breaks every attribute onto its own line, hugging the tail after the
closing `>` — see `prettier_variant_attrs_hug.svelte` (prettier's stable form, which tsv normalizes
back to `input.svelte`).

tsv: word stays hugged, the whole element moves to its own line with attributes intact (each line
≤100), trailing text on its own line
Prettier: keeps `and <a` on the text line and breaks the element internally (one attribute per line),
hugging the tail onto the closing `>`

This pins **element/component parity for the after-element fold's drop coupling**: unlike
`inline_element_wide_long` (whose single long attribute overflows even on its own line, so the drop
falls out of the content-overflow path), here the element's attributes *fit* on its own line, so it
drops whole only because the preceding text is too long — the same case the component fixture
`inline_component_wide_multiattr_long` covers. The trailing text takes its own line for both the
same-line authoring (`unformatted_ours_compact.svelte`) and a newline authoring, so there is no
authoring-dependent split.

## Reason

Print width. tsv treats printWidth as a hard limit and keeps the element intact rather than
splitting its attributes, so an over-wide element goes to its own line; a dropped inline child owns
its line, so following text wraps to the next line rather than hugging the dropped element's `>`. See
[conformance_prettier.md §Svelte: Elements (Wide inline child own-line)](../../../../../docs/conformance_prettier.md#svelte-elements).
