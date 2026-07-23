# inline_break_before_gt_dangle_long_prettier_divergence

The glued-inline-element-chain case (G2) of the break-before rule (see
`inline_break_before_wrap_long`). When an inline element is glued (no whitespace) to a
**following** inline element whose own opening tag must wrap, tsv dangles the *first*
element's closing `>` onto the second element's line — `</span` on one line, `><a` on the
next — so no opening tag is stranded at a line end after a space. The whole glued run first
breaks to a fresh line at the whitespace boundary before it.

This reuses the closing-`>` dangle tsv already applies when an inline element is glued to a
following multiline block (§Svelte: Blocks, sibling `>` dangle). The `>` moves only inside
the end tag (`</span⏎>`), so the boundary whitespace is tag-internal and the output parses to
a byte-identical AST — render-safe (confirmed by `ast_diff --render`).

tsv: `</span`'s `>` dangles down; `<a` leads the continuation line and wraps its attributes.
Prettier: keeps `<span>foo</span><a` glued on one line and dangles the `<a>` attributes/`>`
instead — see `output_prettier.svelte`. `unformatted_ours_compact.svelte` is the compact
authoring (tsv → `input.svelte`, prettier → a dangled form, never `input.svelte`).

## Reason

Design choice. An opening tag never sits at a line end after a space; the closing-`>` dangle
is the render-safe way to give the second element's opening tag its own line when the two are
glued. Render-free under Svelte 5 (whitespace boundary) / tag-internal (the `>` move).
See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
