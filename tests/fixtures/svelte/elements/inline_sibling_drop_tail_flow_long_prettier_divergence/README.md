# inline_sibling_drop_tail_flow_long_prettier_divergence

tsv: the text run **after** an inline element decides its own boundary per width, measured from the
closing tag's actual column — so the tail flows onto that line when it fits there, whatever sibling
precedes the element. The **comment's** authored line is left alone, so the hugged and own-line
spellings are two fixed points. Prettier: agrees on both of those, but also preserves the *tail's*
authored newline, which tsv converges.

## Reason

An inline element preceded by an inline sibling across a collapsible boundary carries that boundary
as its own group (the inline-sibling wrap). A **non-terminal** text run after the element — one with
a further sibling behind it, here a control-flow block — adds a second boundary. The two are
independent decisions and are measured as such: the tail's boundary is the text fill's own `line`,
answered from wherever the closing tag ended up.

Fusing the two boundaries instead resolves them *outside-in*: measuring element
and tail as one unit pushes the leading boundary over, and the tail then rides that break.
That buys a single fixed point for the comment boundary at the price of a line, but it is
conditioned on a property its own output destroys — the wrap exists only while the sibling and the
element share a line, and breaking that line is the fused measurement's whole action. Where the
element stays intact the fused and per-width answers agree by arithmetic; where the element is
wide enough to lay its own content out block-style they do not, and the document formats to one
form and reformats to the other forever
([inline_sibling_drop_tail_wide_long](../inline_sibling_drop_tail_wide_long_prettier_divergence/)
pins that razor).

Retiring the fusion gives up no convergence the project wanted. A comment's authored line is
**authorship** ([§Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)),
so the hugged and own-line spellings are both legitimate stable forms — which this fixture already
held at its fitting width (`variant_control_own_line`) and the fusion overrode only once the run
wrapped. It also puts the two non-flowing siblings (a comment, a control-flow block) back in step
with the flowing ones (an element, a tag, a block element), which always took the per-width answer.

What tsv still converges is the **tail** boundary, and that is the divergence here: the whitespace
between a closing tag and the text after it is render-free under Svelte 5, so it carries no
authorship to preserve, and both spellings reach one form. Prettier holds a distinct stable form for
each.

## Cases

- **100 (control)** — the comment, the element and the first word fit exactly, so the element hugs
  the comment's line and the run wraps after it. Both formatters agree.
- **101** — the same document one character wider: the element no longer fits on the comment's line,
  so it takes its own line and the tail flows onto the closing tag. Both formatters agree.

`variant_comment_hug` authors the 101 document with the comment on the element's line: a **second**
fixed point, held by both formatters, since the comment's line is authorship rather than layout.
`variant_control_own_line` is the same for the 100 document. `prettier_variant_own_line` writes the
101 document with the tail on its own line as well — prettier keeps that third form, tsv converges
it back to `input`, and that boundary is the sole divergence. `unformatted_tail_split` breaks the
101 case's tail mid-run instead of at the boundary; both formatters repack it, pinning that the
divergence is confined to the element↔text boundary and not the run-interior reflow.

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
