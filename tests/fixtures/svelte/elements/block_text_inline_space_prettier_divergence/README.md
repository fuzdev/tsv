# block_text_inline_space_prettier_divergence

**◆prettier_bug ◆content_preservation.** A text that follows a block element keeps the space
before the inline element after it: `<div>block1</div>⏎text1 text2 <span>inline1</span>` renders
`text1 text2 inline1`. Prettier deletes that space (`text1 text2<span>inline1</span>`,
`output_prettier.svelte`), which renders `text1 text2inline1` — a content change, in a run that
continues past the element, for a one-word text, and whether or not the run is the fragment's
first. A tag or a component after the same text keeps its space under both
formatters (the controls).

The deletion is keyed on the **text's predecessor**, not on the run, so one run gets two answers:
in `text1 <span>inline1</span> text2 <span>inline2</span> text3` after a block, only the first
boundary loses its space. Every block predecessor kind answers alike (a whitespace-sensitive
`<pre>` and a void `<hr />` are pinned beside the `<div>`), and so does every inline follower kind
— including a **void** (`<br />`) and a **replaced** (`<input />`) element, where the loss is
invisible to `render_compare`'s render key (it tracks text runs, and neither element contributes
one) but present in the compiled HTML: `text1 text2 <input/>` vs `text1 text2<input/>`.

## Reason

prettier-plugin-svelte's `handleTextChild` trims the text's trailing whitespace before an inline
element and hands the element a `group([line, …])` to re-emit it — except when the text's
predecessor is a block element, where it still trims but sets the flag that emits the `line` to
false (`handleWhitespaceOfPrevTextNode = !isBlockElement(prevNode)`), so the space is gone with
nothing to print it. tsv's block boundary already supplies the break after the block, and the
text's trailing space before the inline element is inter-sibling whitespace the compiler renders,
so it is kept as the element's per-width wrap like everywhere else.

## Files

- `unformatted_ours_compact.svelte` — the same-line authoring of each block boundary
  (`<div>block1</div> text1 text2 <span>inline1</span>`); tsv trims that leading run
  ([space_after_block](../space_after_block_prettier_divergence/)) and keeps the trailing space,
  reaching `input.svelte` in one pass.
- `unformatted_ours_tab.svelte` — the space before the inline element spelled as a tab; tsv
  normalizes it to the space. Prettier deletes it the same way it deletes the space.

See
[conformance_prettier_svelte.md §Svelte: Elements](../../../../../docs/conformance_prettier_svelte.md#svelte-elements).
