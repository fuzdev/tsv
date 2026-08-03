# inline_sibling_newline_flow_prettier_divergence

tsv: an inline sibling isolated by authored newlines flows back onto the content line — the
authorings converge on the **multiline** form. Prettier: keeps a distinct stable form for each.

## Reason

**Design choice — authoring-independence, on one axis only.** Two independent questions live at
an inline sibling's newline, and tsv answers them differently:

- **Does the element lay out multiline?** An authored newline anywhere in the content says yes,
  and that is *preserved* — "want air, author multiline". The fully hugged authoring stays
  hugged (both forms are shared fixed points; prettier agrees). Not touched here.
- **How are the siblings separated once multiline?** Svelte 5 collapses inter-sibling whitespace
  to one whitespace, so a newline and a space between two siblings render identically. The
  newline's *spelling* carries no signal, so the siblings flow onto one content line.

So `text1⏎<span>inline1</span>⏎text2` and `text1 <span>inline1</span> text2` inside a multiline
element are one document and converge; prettier lets each boundary decide and holds a stable
form for each. Crucially the separator's **presence** still decides layout — a *glued* boundary
is never split, since breaking there would inject a rendered space. Only space↔newline is
reshaped, never space↔nothing.

This is the inter-sibling analog of the content-boundary convergence
[inline_boundary_whitespace_multiline](../inline_boundary_whitespace_multiline_prettier_divergence/)
pins, and it turns the large majority of `authoring_audit`'s `content-leading` /
`content-trailing` sites from `diverge (dual-stable)` to `converge`.

## Cases

An inline element, a component, an expression tag, a render tag and a control-flow block, each
in the converged multiline form, with the three isolated authorings (newline before / after /
both) as `prettier_variant_*` files.

## Controls — what does NOT flow

- **The fully hugged authoring stays hugged.** The element's multiline-ness is a separate axis
  and is preserved, so collapsing the multiline cases onto one line would destroy that signal.
- **A comment keeps its authored line.** A comment's position is authorship, so folding one into
  a text fill would relocate it across a semantic boundary — the one thing
  [§Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
  exists to prevent. Each side of a comment is held independently, and the hugged authoring is
  held too — [inline_separator_comment_newline](../inline_separator_comment_newline/) pins those
  four authorings, where both formatters agree.
- **A blank line still breaks.** Blank-line preservation is a Tier-2 authoring signal
  independent of render, uniform with every other boundary tsv produces.
- **A block sibling still takes its own line**, while the inline run before it flows — blocks
  merely partition a fragment into inline runs, each of which flows on its own.

## The boundary of those controls — bounding a run is not sterilizing it

⚠️ A sibling that owns its own line **ends** the run; it does not stop the run's *other*
boundaries from flowing. Stated only in prose, that half is invisible: a control whose run is
already hugged in every file is a fixed point under either rule, so it cannot tell "the comment
holds its own two boundaries" apart from "a run anywhere near a comment keeps every line" — and
the second reading is a different rule that happens to satisfy the first control.

So two controls carry an inline run **beside** the bounding sibling and re-author it in all three
variants, which makes the flow a normalization the fixture actually observes: the comment control's
twin holds a run on *each* side of the comment, and the block-sibling control's run before the
block is isolated per variant. Both converge on `input` while prettier keeps each spelling —
exactly the divergence the main cases pin, now measured across a run boundary.

See
[conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
