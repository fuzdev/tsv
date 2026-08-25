# inline_interior_newline_label_hold_prettier_divergence

The prose gate read by the **interior** side of the render-free content boundary: whether an
authored newline inside a hugged inline element may select its layout. A run holding a single
word is a label with no fill to reflow into, so its whitespace-only newline is held
— and holding it expands the element (`<span>⏎<code>a</code>⏎<code>b</code> text1⏎</span>`),
since the hugged form has nowhere to put a hardline. Two one-word texts in one run are two labels,
not a phrase — the count is per node, never a sum — so
`<span><code>a</code>⏎<code>b</code> text1 <code>c</code> text2</span>` expands the same way. The
same newline in a run that is prose — one text carries two words — is the fill's own wrap point
and the element collapses (`<span>{a} {b} text1 text2</span>`).

## Reason

Design choice, the same "is there a fill?" answer
[inline_content_flow_collapse](../inline_content_flow_collapse_prettier_divergence/) gives for a
prose run, read with the prose gate of
[inline_sibling_newline_label_hold](../inline_sibling_newline_label_hold_prettier_divergence/):
every reader of "did the author break this content?" must consult one answer, or a wide fill
writes the very newline the other reader keys on and the two forms alternate. With no fill the
authored newline is the author's only structure, so the interior arm holds it exactly as the
sibling arms do.

`unformatted_ours_hugged.svelte` is the hugged authoring of every case. tsv normalizes it to
`input.svelte` in one pass; prettier keeps the hug and dangles the closing delimiter
(`<span⏎><code>a</code>⏎<code>b</code> text1</span⏎>`) — its interior newline is held but never
selects the element's layout.

⚠️ The hold reaches only the whitespace-only separator. The content-text arm's edge trim still
collapses the newline when it sits at a text node's edge (`<span>text1⏎<code>a</code></span>`
→ `<span>text1 <code>a</code></span>`), so the two spellings of a one-word interior newline do
not yet agree; that arm is tracked as open work and deliberately not pinned here.

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
