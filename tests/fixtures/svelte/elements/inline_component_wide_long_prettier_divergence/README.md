# inline_component_wide_long_prettier_divergence

A wide inline component (`<Comp href=…>`) whose opening tag is too wide to share a
line with the preceding text. tsv drops the whole component to its own line and keeps
the preceding word (`and`) hugged on the text line. Prettier instead hugs the component
onto the text line and breaks its attributes / dangles the closing `>` (the inline
content hug).

tsv: word stays hugged, the whole component moves to its own line (each line ≤100)
Prettier: keeps `and <Comp` on the text line (101) and breaks the component internally
— see `prettier_variant_attrs_hug.svelte` (prettier's stable form, which tsv normalizes
back to `input.svelte`).

This is the **wide** counterpart of `inline_component_fill_long` (a short component that
flows within the fill). The point here is idempotence: feeding the compact/space-authored
form (`unformatted_ours_compact.svelte`) must normalize to `input.svelte` in one pass —
the preceding word must not be stranded onto its own line.

## Reason

tsv treats printWidth as a hard limit and keeps the component intact rather than splitting
its attributes/closing `>`, so an over-wide component goes to its own line. The boundary
before the component is a collapsible space, so the word before it stays on the text line.
See [conformance_prettier.md §Inline content hug](../../../../../docs/conformance_prettier.md#svelte-elements).
