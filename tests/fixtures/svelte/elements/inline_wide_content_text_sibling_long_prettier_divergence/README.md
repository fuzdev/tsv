# inline_wide_content_text_sibling_long_prettier_divergence

The companion to `inline_wide_content_trailing_long`: a wide inline element whose **prose content**
overflows, but here the following text (`mid`) is **non-terminal** — another inline element
(`<b>`) follows it.

tsv lays the element out **block-style** (both tags intact, the over-wide content wrapped within
printWidth — prettier keeps it on one over-width dangled line), and the non-terminal text **hugs
the intact closing tag**: `</a> mid <b>x</b>`. The tail boundary after an inline element is a
per-width fill decision measured from the closing tag's own column, however the element came to be
multiline — this fixture pins the prose-content case of the same rule
`inline_wide_element_content_tail_long` pins for element-child content at the exact 100/101
boundary, and the terminal case (`inline_wide_content_trailing_long`) hugs under the same
principle.

The **second case** swaps the preceding text for an inline **element** sibling (`<span>a</span>`),
so the wide element sits in its inline-sibling wrap. What sibling kind precedes the wide element
cannot change where its tail breaks: the wrap's other side *flows* (an element's separator
spellings converge), so the tail takes the same per-width hug. The one shape that instead keeps
the joint element+boundary measurement is a **text-only** element in a wrap whose other side
does **not** flow — a spaced comment or a control-flow block, whose own-line spelling is a fixed
point and therefore the only convergeable target — where two boundaries meet on one element and
resolve outside-in; see `inline_sibling_drop_tail_flow_long`, and
`inline_comment_wrap_fill_tail_long` for why any other content kind takes the per-width answer
even there (its width-broken form regenerates the static trigger).

The `unformatted_ours_*` variants pin idempotence: the single-line and one-line-content authorings
both normalize to the hugged form in one pass.

## Prettier's forms

Prettier groups the tail boundary WITH the element, so once the element is multiline the tail
always drops — it never holds the hugged form:

| file | authoring | claim |
| --- | --- | --- |
| `output_prettier.svelte` | prettier from `input` | keeps the block-style content (source newlines) but re-breaks the hugged tail to its own line; a form it keeps stable, which tsv folds back to `input` |
| `unformatted_ours_compact.svelte` | everything on one line | tsv → `input` in one pass; prettier does not reach `input` from it |
| `unformatted_ours_multiline.svelte` | content on one line inside a multiline `<p>` | tsv → `input` in one pass |
| `prettier_variant_compact.svelte` | prettier from the compact authoring | dangles the tag delimiters around the over-width content (`>…</a⏎>`) with the tail on its own line; prettier keeps it, tsv → `input` |

The boundary tsv folds is inter-node whitespace that renders as one space either way, so the
output renders identically to the input.

## Reason

Two deliberate choices:

1. **Block-style content** — tsv keeps printWidth a hard limit and lays the element out block-style
   (both tags intact, content on its own indented line) rather than emitting prettier's single
   over-width dangled line.
2. **The non-terminal tail hugs per width** — the whitespace between the closing tag and the tail
   is render-free under Svelte 5, so tsv converges its spellings onto the per-width answer — hug
   when it fits, break when it does not — where prettier's element-grouped boundary always breaks
   once the element is multiline, holding a distinct stable form per authoring.

See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
