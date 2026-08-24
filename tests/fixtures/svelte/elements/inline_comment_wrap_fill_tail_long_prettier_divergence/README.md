# inline_comment_wrap_fill_tail_long_prettier_divergence

The fill-content twin of
[inline_sibling_drop_tail_flow_long](../inline_sibling_drop_tail_flow_long_prettier_divergence/):
the wrap's other side is a comment, whose own-line spelling is pinned — but the element's
content holds prose and a child, so its width-broken block-style form re-parses as the
authored-air document. The width-broken and newline-authored *content* spellings are ONE document, and
the non-terminal tail's **space** spelling after the element must answer identically in both: per
width, from the closing tag's own column — the same rule
[inline_wide_content_text_sibling_long](../inline_wide_content_text_sibling_long_prettier_divergence/)
pins without the wrap. A joint element+boundary measurement has
no stable answer here: width-breaking the element *creates* the newline-authored document, whose
own pass hugs the tail per width, so a joint first pass reaches the fixed point only on the
second pass — the regeneration invariant outranked the outside-in preference for any element
whose content is not text-only. The joint measurement has since been retired for the text-only
twin as well, on the same argument one width further out
([inline_sibling_drop_tail_wide_long](../inline_sibling_drop_tail_wide_long_prettier_divergence/)),
so every non-terminal tail's space spelling now answers per width.

## Cases

- **100** — the element block-styles and the non-terminal tail (a prose run with a trailing
  `<b>` sibling) hugs the intact closing tag at exactly print width.
- **101** — the same document one character wider: the trailing element wraps to its own line
  and the prose keeps the hug.

`unformatted_ours_same_line` authors both cases on one line; tsv normalizes it to `input` in one
pass. Prettier instead drops the tail to its own line once the element is multiline and holds a
stable form per authoring: `output_prettier.svelte` is its form from `input` (tail dropped, the
authored block layouts kept), `variant_dropped_tail` its form from the dropped-tail authoring —
now **dual-stable**, since a tail's authored newline after a multiline-rendering unwrapped
element is preserved (the layout-keyed rule; in these geometries the comment has its own line, so
the element carries no wrap) — and `divergent_variant_dangle` its form from the one-line
authoring (the tag delimiters dangled, the tail dropped), which tsv rewrites to the dropped-tail
form rather than to `input`.

## Reason

Design choice, the same one the unwrapped fixtures record: the tail boundary's **space**
spelling after an inline element is a per-width fill decision measured from the closing tag's
own column, however the element came to be multiline (an authored newline is layout-keyed
instead — preserved beside a multiline-rendering unwrapped element, as `variant_dropped_tail`'s
dual-stability records), and every space-spelled authoring of the document converges on one
fixed point. The boundaries tsv moves are render-free under Svelte 5.
See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
