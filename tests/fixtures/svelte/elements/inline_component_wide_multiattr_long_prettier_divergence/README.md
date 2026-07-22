# inline_component_wide_multiattr_long_prettier_divergence

The breakable-inner-content counterpart of `inline_component_wide_long`: a wide self-closing
component with several attributes (each of which *could* break onto its own line) is too wide
to share a line with the preceding text. tsv drops the whole component to its own line, where
its attributes still fit on one line, keeps the preceding word (`and`) hugged, and lets the
**space-authored** trailing text flow after the intact `/>`. Prettier instead hugs `and <Comp`
onto the text line (101) and breaks every attribute onto its own line — see
`prettier_variant_attrs_hug.svelte` (prettier's stable form, which tsv normalizes back to
`input.svelte`).

tsv: word stays hugged, the whole component moves to its own line with attributes intact (each
line ≤100), and the space-authored tail (`tail1 tail2 tail3`) flows after the intact `/>` — a
short dropped child packs its trailing text like any other fill word.
Prettier: keeps `and <Comp` on the text line and breaks the component internally (one attribute
per line)

This complements `inline_component_wide_long` (a single long attribute): the flow boundary keys
on the component being too wide for the *text* line, not on whether the component's own
attributes can break.

The trailing text follows the **authored boundary**, exactly like the wide-content case
(`inline_wide_content_trailing_long`): a space flows after the intact `/>` (both
`unformatted_ours_compact.svelte` and prettier's hugged form normalize to this input), a newline
keeps it on its own line — the drop no longer isolates its tail regardless of authoring.

## Reason

Print width plus the after-element fold. tsv treats printWidth as a hard limit and keeps the
component intact rather than splitting its attributes, so an over-wide component goes to its own
line — the **sole divergence** from prettier's internal attribute break. The boundary before the
component is a collapsible space, so the word before it stays on the text line; the short dropped
component then packs its trailing text like every other fill word (the tail flows after `/>`). See
[conformance_prettier.md §Svelte: Elements (Wide inline child own-line)](../../../../../docs/conformance_prettier.md#svelte-elements).
