# inline_tag_pair_space_prettier_divergence

A **space-spelled** separator between two tags — `{expr1} {expr2}` — is a fill boundary that
packs per width in a multiline container, whatever its run holds: no word at all, one word
(`text1 {expr1} {expr2}`, `{expr1} {expr2} {expr3} text1`, the escaped-angle-bracket pair at the
root), or prose. An inline element or a component before the tag (`<span>inline1</span> {expr}`,
`<Comp /> {expr}`), a component between two tags, and every tag kind in one run
(`{@html a} {@render fn()} {expr}`) pack the same way. Prettier's bare `line` between two tags
breaks with the multiline container, so it splits every one of them (`output_prettier.svelte`).
The control is the **newline** spelling of the wordless pair, which both formatters hold.

`unformatted_ours_tab.svelte` spells every packed separator as a tab: a tab and a space are one
document ([inline_separator_tab](../inline_separator_tab_prettier_divergence/)), so tsv
normalizes it to `input.svelte`; prettier splits the pairs as it does from the space.

`variant_split.svelte` is prettier's form itself, and tsv keeps it stable too — the newline spelling of
every boundary is held — so the two forms are two fixed points; a rule change that started
normalizing prettier's form back to `input.svelte` fails this file rather than passing in prose.

## Reason

Design choice. The sibling-newline flow rule's prose gate
([inline_sibling_newline_label_hold](../inline_sibling_newline_label_hold_prettier_divergence/))
is a **hold**, never a forced break: it decides whether an authored newline may reflow, and says
nothing about an authored space. So the whitespace-only separator before a tag reads the prose
gate for its NEWLINE spelling alone — a wordless or one-word run holds its authored lines — and
its SPACE spelling defers to the next tag's per-width group unconditionally, the same
`group([line, tag])` an inline element or component takes at that boundary
([inline_adjacent_sibling_newline_flow](../inline_adjacent_sibling_newline_flow_prettier_divergence/),
the prose tag pair prettier also splits). Gating the space on what the run holds would turn the
hold into a forced break — `Price: {currency} {amount}`, or a bare `{a} {b}`, authored on one
line and coming out on two — a layout keyed on nothing the author wrote, and the one place the
rule family used to do it. Both arms of the separator site defer the tag's space to that
per-width group, so a width-broken container and a newline-authored one lay the pair out
identically; and the space asks nothing about what precedes it either — a comment, a `<br />` or
a control-flow block before the pair keeps the space exactly as it would before an element
([inline_tag_pair_space_bounded](../inline_tag_pair_space_bounded_prettier_divergence/)).

The container is not an axis either —
[inline_tag_pair_space_container](../inline_tag_pair_space_container_prettier_divergence/); the
pair packs per width at the print-width boundary in
[inline_content_spaced_tags_long](../inline_content_spaced_tags_long_prettier_divergence/).

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
