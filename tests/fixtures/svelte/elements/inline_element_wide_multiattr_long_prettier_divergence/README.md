# inline_element_wide_multiattr_long_prettier_divergence

The HTML-element counterpart of `inline_component_wide_multiattr_long`: a wide inline element with
several attributes (each of which *could* break onto its own line) is too wide to share a line with
the preceding text. tsv drops the whole element to its own line, where all its attributes still fit
on one line, keeps the preceding word (`and`) hugged, and lets the **space-authored** trailing text
(`tail1 tail2 tail3`) flow after the intact `</a>` — a short element (its content fits) packs like
any other fill word. Prettier instead hugs `and <a` onto the text line and breaks every attribute
onto its own line, hugging the tail after the dangled `>` — see `prettier_variant_attrs_hug.svelte`
(prettier's stable form, which tsv normalizes back to `input.svelte`).

tsv: word stays hugged, the whole element moves to its own line with attributes intact (each line
≤100), the space-authored tail flows after `</a>`
Prettier: keeps `and <a` on the text line and breaks the element internally (one attribute per line),
hugging the tail onto the dangled `>`

This pins **element/component parity for the after-element fold**: unlike `inline_element_wide_long`
(whose single long attribute overflows even on its own line, so the drop falls out of the
content-overflow path), here the element's attributes *fit* on its own line, so it drops whole only
because the preceding text is too long — the same case the component fixture
`inline_component_wide_multiattr_long` covers. The trailing text follows the **authored boundary**,
exactly like the wide-content case (`inline_wide_content_trailing_long`): a space flows after the
intact `</a>` (both `unformatted_ours_compact.svelte` and prettier's hugged form normalize to this
input), a newline keeps it on its own line — the drop no longer isolates its tail regardless of
authoring.

## Reason

Print width plus the after-element fold. tsv treats printWidth as a hard limit and keeps the element
intact rather than splitting its attributes, so an over-wide element goes to its own line — the
**sole divergence** from prettier's internal attribute break + dangle. A *short* dropped element then
packs its trailing text like every other fill word (the tail flows after the intact `</a>`), mirroring
how `<el>x</el> tail` already stays inline; the block-style layout is the divergence, not the tail
placement. See
[conformance_prettier.md §Svelte: Elements (Wide inline child own-line)](../../../../../docs/conformance_prettier.md#svelte-elements).
