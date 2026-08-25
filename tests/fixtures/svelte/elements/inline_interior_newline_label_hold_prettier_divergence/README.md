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

The hold reaches **both spellings of the newline**, which is the point: an authored newline
between two inline siblings is folded by the parser into a whitespace-only node when nothing else
sits there, and into the edge whitespace of the content text beside it when something does. One
document must not get two layouts by that accident — the same reason the run scan bounds a run at
an authored blank line wherever the parser put it
([inline_sibling_newline_run_bounded](../inline_sibling_newline_run_bounded_prettier_divergence/)).
So `<span>text1⏎<code>a</code></span>`, `<span><code>a</code>⏎text1</span>` and
`<span><code>a</code> <code>b</code>,⏎<code>c</code></span>` hold exactly as the whitespace-only
cases above do; a lone `,` is a word like any other, since a word is a run of non-collapsible
whitespace and nothing else
([inline_sibling_newline_prose_words](../inline_sibling_newline_prose_words_prettier_divergence/)).
Prettier holds the newline at both spellings too, so the hold is agreement and the divergence
stays the dangle-vs-block-style spelling this fixture already owns.

The control is the two-word twin of the same edge (`<span>text1 text2 <code>a</code></span>`):
the run is prose, the newline is the fill's own wrap point, and the element collapses — the
positive case [inline_content_flow_collapse](../inline_content_flow_collapse_prettier_divergence/)
is built on.

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
