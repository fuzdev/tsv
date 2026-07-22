# inline_boundary_whitespace_multiline_prettier_divergence

The multiline counterpart of
[inline_boundary_whitespace](../inline_boundary_whitespace_prettier_divergence/), which covers an
inline element whose content stays inline (boundary runs trimmed there too).

Once the children force multiline (a block child, a control-flow block), the element lays out
**block-style** — both tags intact, content on its own indented lines — and the content-boundary
whitespace is trimmed. Prettier instead lets that whitespace pick the layout, dangling the tag
delimiters when the author hugged a boundary (`<div>block</div></span⏎>`).

Content-boundary whitespace is render-free under Svelte 5: whitespace at the start and end of a tag
is removed at compile, so `<span>text…`, `<span> text…`, and a newline boundary are the **same
document**. tsv converges all three on one form; prettier produces a different stable form for each.
The `unformatted_ours_*` variants are those authorings — `_hug` (no boundary whitespace) and
`_space` (a space boundary) — both of which tsv normalizes to the block-style input while prettier
does not.

Preserving the space here is not merely a divergence but a defect: emitted at line-start it renders
as nothing, re-parses as indentation, and is dropped on the next pass — a non-idempotent format.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
