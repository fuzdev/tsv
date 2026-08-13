# inline_sibling_drop_tail_flow_long_prettier_divergence

tsv: when the boundary with an inline sibling drops an element to its own line, the text run
**after** the element still decides its own boundary — so the tail flows onto the closing tag's
line when it fits there. Prettier: holds a distinct stable form for each authoring.

## Reason

An inline element preceded by an inline sibling across a collapsible boundary carries that
boundary as its own group (the inline-sibling wrap). A **non-terminal** text run after the element
— one with a further sibling behind it, here a control-flow block — adds a second boundary. The
two are independent decisions and must be measured as such: once the leading boundary breaks the
element starts a fresh line, and the next pass measures the trailing boundary *from that fresh
line*. Welding both boundaries into one group measures the trailing one against the pre-break
column instead — a column that no longer exists in the output — so the same document reaches one
form on the first pass and another on the second, and has no fixed point.

The trailing boundary stays grouped **with the element**, because a wide element that wraps its own
content must still push its tail onto the next line; only the leading boundary is hoisted out. This
is the non-terminal counterpart of the terminal fold, which already separates the same two
boundaries before folding element and tail into one fill
([inline_element_wide_multiattr_long](../inline_element_wide_multiattr_long_prettier_divergence/)).

The joint grouping is scoped to a **text-only** element like this one, the one content kind whose
width-broken form regenerates no static trigger. Any other content re-parses its emitted boundary
newlines as the authored-air signal, so its tail must answer per width even inside the wrap — see
[inline_comment_wrap_fill_tail_long](../inline_comment_wrap_fill_tail_long_prettier_divergence/),
the fill-content twin.

## Cases

- **100 (control)** — the comment, the element and the first word fit exactly, so the element hugs
  the comment's line and the run wraps after it.
- **101** — the same document one character wider: the leading boundary breaks, the element takes
  its own line, and the tail flows onto the closing tag's line because it fits there.

`unformatted_ours_same_line` authors both cases with the comment on the element's line;
`prettier_variant_own_line` authors the 101 case with the comment and the tail each on their own
line; `unformatted_tail_split` breaks the 101 case's tail mid-run instead of at the boundary —
prettier repacks that one too, pinning that the divergence is confined to the element↔text
boundary, not the run-interior reflow. All normalize to input, so every authoring of the 101
document reaches **one** fixed point. Prettier keeps a separate stable form for each boundary
authoring.

At fitting widths the convergence deliberately stops at the comment boundary: both spellings are
comment authorship (§Comment Position Philosophy), so the 100 document has **two** fixed points —
input's hug, and `variant_control_own_line` (the control with the comment on its own line, the
freed room repacking the tail), which both formatters hold stable.

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
