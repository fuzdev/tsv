# inline_comment_wrap_fill_tail_long_prettier_divergence

The fill-content twin of
[inline_sibling_drop_tail_flow_long](../inline_sibling_drop_tail_flow_long_prettier_divergence/):
the wrap's other side is a comment, whose own-line spelling is pinned — but the element's
content holds prose and a child, so its width-broken block-style form re-parses as the
authored-air document. The width-broken and newline-authored spellings are ONE document, and
the non-terminal tail after the element must answer identically in both: per width, from the
closing tag's own column — the same rule
[inline_wide_content_text_sibling_long](../inline_wide_content_text_sibling_long_prettier_divergence/)
pins without the wrap. The joint element+boundary measurement the text-only twin keeps has no
stable answer here: width-breaking the element *creates* the newline-authored document, whose
own pass hugs the tail per width, so a joint first pass reaches the fixed point only on the
second pass — the regeneration invariant outranks the outside-in preference for any element
whose content is not text-only.

## Cases

- **100** — the element block-styles and the non-terminal tail (a prose run with a trailing
  `<b>` sibling) hugs the intact closing tag at exactly print width.
- **101** — the same document one character wider: the trailing element wraps to its own line
  and the prose keeps the hug.

`unformatted_ours_same_line` authors both cases on one line; tsv normalizes every authoring to
`input` in one pass. Prettier instead drops the tail to its own line once the element is
multiline and holds a stable form per authoring, so the authorings never converge there:
`output_prettier.svelte` is its form from `input` (tail dropped, the authored block layouts
kept), `prettier_variant_dropped_tail` its form from the dropped-tail authoring (which is its
own fixed point), and `prettier_variant_dangle` its form from the one-line authoring (the tag
delimiters dangled, the tail dropped) — tsv normalizes both variants to `input`.

## Reason

Design choice, the same one the unwrapped fixtures record: the tail boundary after an inline
element is a per-width fill decision measured from the closing tag's own column, however the
element came to be multiline, and every render-free authoring of the document converges on one
fixed point. The boundaries tsv moves are render-free under Svelte 5.
See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
