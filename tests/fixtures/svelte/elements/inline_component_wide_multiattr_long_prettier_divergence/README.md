# inline_component_wide_multiattr_long_prettier_divergence

The breakable-inner-content counterpart of `inline_component_wide_long`: a wide self-closing
component with several attributes (each of which *could* break onto its own line) is too wide
to share a line with the preceding text. tsv drops the whole component to its own line, where
its attributes still fit on one line, and keeps the preceding word (`and`) hugged. Prettier
instead hugs `and <Comp` onto the text line (101) and breaks every attribute onto its own line
— see `prettier_variant_attrs_hug.svelte` (prettier's stable form, which tsv normalizes back
to `input.svelte`).

tsv: word stays hugged, the whole component moves to its own line with attributes intact (each
line ≤100), and the trailing text (`tail1 tail2 tail3`) wraps to its own line — a wide inline
child that drops owns its line, so following text doesn't hug its `>`.
Prettier: keeps `and <Comp` on the text line and breaks the component internally (one attribute
per line)

This complements `inline_component_wide_long` (a single long attribute): the flow boundary keys
on the component being too wide for the *text* line, not on whether the component's own
attributes can break.

The trailing text takes its own line whether the boundary was authored as a space
(`unformatted_ours_compact.svelte`) or a newline — both converge here, so there is no
authoring-dependent dual-stable split (the after-element fold breaks the trailing separator when
the dropped element renders at line start).

## Reason

tsv treats printWidth as a hard limit and keeps the component intact rather than splitting its
attributes, so an over-wide component goes to its own line. The boundary before the component is
a collapsible space, so the word before it stays on the text line. See
[conformance_prettier.md §Inline content hug](../../../../../docs/conformance_prettier.md#svelte-elements).
