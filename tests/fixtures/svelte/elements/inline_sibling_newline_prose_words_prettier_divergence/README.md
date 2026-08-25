# inline_sibling_newline_prose_words_prettier_divergence

What the sibling-newline flow rule counts as a **word** when it asks whether a run is prose
([inline_sibling_newline_label_hold](../inline_sibling_newline_label_hold_prettier_divergence/)):
a content text's collapsible-whitespace-separated words, split over the **source bytes** —
exactly the split the fill makes when it turns the text into items. So an NBSP-joined pair is one
word (one fill item), and so is an entity-encoded space (`text1&#32;text2`: the entity's bytes
print verbatim, so the fill carries the pair as one unbreakable item — prettier's does too — and
there is no seam to reflow at); punctuation alone is a word, a hyphenated pair is one, a word
glued to a tag (`{expr}text1`) is still one, and punctuation alone between two tags (`{a}⏎·⏎{b}`)
is a one-word connector that holds. One word holds as a label; two is the cliff — at a void
element (`<input /> text1 text2`, the checkbox-and-caption shape where label and prose genuinely
blur) as at any other sibling — and a newline INSIDE a text node separates words exactly as a
space does, so `<Comp />⏎text1⏎text2` is a two-word node and flows to `<Comp /> text1 text2`
(`unformatted_ours_newline_in_node.svelte`, the newline authoring with that node split; prettier
reflows the interior newline but holds the sibling one, landing on `prettier_variant_newline`).

## Reason

Design choice. The count exists to answer one question — is there a fill to reflow into — and a
fill reflows only between its items, so the count splits the same text the fill splits: the
source bytes, never the decoded characters. (The rule's other text question, whether a node is a
separator wearing content's clothing, reads the DECODED text, because that one is a render
question — an NBSP renders as itself; see `inline_separator_nbsp_newline`. The two axes are
deliberately distinct.) Counting decoded characters instead would call `text1&#32;text2` a
two-word phrase and flow the newline beside it on the promise of a wrap point that neither
formatter's fill has. Prettier holds every authored newline here; the held cases are agreement,
and the three controls are the divergence — `prettier_variant_newline.svelte` is their isolated
authoring, kept by prettier and normalized to `input.svelte` by tsv.

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
