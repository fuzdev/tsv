# inline_break_before_gt_dangle_shedder_long_prettier_divergence

The glued-element-pair `>` dangle (G2, `inline_break_before_gt_dangle_long`) seen from the
**shedder**, the first element, which hands its closing `>` to the receiver, when the shedder
itself is too wide for its line. Its content drops to its own indented line and **so does its
closing tag** (`<b class="…">⏎\tx⏎</b><i>x</i>`): the shedder lays out block-style exactly like
an element that keeps its `>`, and only the `>` is handed on. It hugs the receiver's opening tag
when that fits (`</b><i>`) and dangles onto the receiver's line when the receiver's attributes
wrap (`</b⏎><i⏎\tclass="…"`).

The shedder's block-style content is also what a both-boundary newline authoring looks like, so
its role must not depend on how that content was written: a shedder multiline only by authored
line breaks (`<b>⏎{y}⏎</b>`) sheds its `>` exactly like its width-broken twin, so before a
receiver whose attributes wrap it prints `</b⏎><i⏎\tclass="…"` where prettier keeps `</b><i`.
That cell is the element-path twin of the cost the block path accepts (`</b⏎>{#if c}`, pinned in
`inline_sibling_gt_dangle_shedder_long`). A shedder multiline by **structure**, one holding a
block (`{#if x}a{/if}`), keeps its `>`: the control, which prettier prints identically.

The 100/101 boundary is the shedder's flat line up to the receiver's opening tag
(`<b class="…">x</b><i>`): at 100 the shedder stays flat and the receiver goes block-style; at
101 the shedder goes block-style. The shedder may hold an expression tag, an element or a
comment, may be a component, may sit in the middle of a run of three (also receiving its
predecessor's `>` while shedding its own) or behind a glued comment, and the pair may sit at the
root, in a block parent, in an inline parent, or after prose, where it travels to a fresh line
first and the shedder goes block-style there.

- `unformatted_ours_compact.svelte` — every case on one line but for the authored line breaks
  its case names. tsv normalizes it to `input.svelte`; prettier to
  `prettier_variant_hugged.svelte`.
- `unformatted_ours_half_block.svelte` — each broken shedder's content on its own line with its
  closing tag hugging that line (`⏎\tx</b><i>x</i>`), half block-style. tsv normalizes it to
  `input.svelte`; prettier to `prettier_variant_half_block_dangle.svelte`.
- `prettier_variant_hugged.svelte` — prettier's form of the compact authoring: where tsv breaks
  a shedder for width, prettier keeps its content on the tag line and dangles tag delimiters
  instead, mostly the receiver's (`</b><i⏎\t>x</i⏎>`). Prettier-stable; tsv normalizes it to
  `input.svelte`.
- `prettier_variant_half_block_dangle.svelte` — prettier's form of the half block-style
  authoring: the content keeps hugging `</b` and the `>` dangles onto the receiver's line
  (`⏎\tx</b⏎><i>x</i>`). Prettier-stable; tsv normalizes it to `input.svelte`.
- `output_prettier.svelte` — prettier's form of `input.svelte`: every block-style shedder as tsv
  prints it, but `</b><i` joined where tsv dangles the `>`, and a component shedder's content
  hugged with the receiver's delimiters dangled.

Every form renders the same: the `>` moves only inside an end tag, and the content-boundary
whitespace the forms differ in is trimmed at compile.

## Reason

Design choice. An inline element whose content goes multiline lays out block-style, both tags
intact on lines of their own, and handing its `>` to a glued sibling does not exempt it: a
content line with the closing tag hugged to it is neither the flat nor the block-style form.
Which elements shed is decided by *cause*, because a shedder's block-style output is
byte-for-byte the authored-multiline form: a role keyed on authored line breaks would be
re-decided on the next pass, while one keyed on structure cannot be. See
[conformance_prettier_svelte.md §Moving a run: break-before, travel, and welded units](../../../../../docs/conformance_prettier_svelte.md#moving-a-run-break-before-travel-and-welded-units)
and
[§Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).

## Related

- [elements/inline_break_before_gt_dangle_long](../inline_break_before_gt_dangle_long_prettier_divergence/) — the G2 pair with a text receiver
- [elements/inline_break_before_gt_dangle_nontext_long](../inline_break_before_gt_dangle_nontext_long_prettier_divergence/) — the receiver side: non-text receiver content
- [elements/inline_break_before_chain_long](../inline_break_before_chain_long_prettier_divergence/) — G2 over a run of 3+ glued elements
- [elements/inline_sibling_gt_dangle_shedder_long](../inline_sibling_gt_dangle_shedder_long_prettier_divergence/) — the same shedder before a block
