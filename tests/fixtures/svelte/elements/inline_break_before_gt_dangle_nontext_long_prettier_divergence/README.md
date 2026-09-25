# inline_break_before_gt_dangle_nontext_long_prettier_divergence

The glued-element-pair `>` dangle (G2, `inline_break_before_gt_dangle_long`) when the
**receiver** — the second element, whose opening tag must wrap — holds non-text content: an
expression tag, an element, a comment, or text and a tag. tsv dangles the shedder's closing
`>` onto the receiver's line (`<b>a</b` / `><i` / its wrapped attributes / `>`) and lays the
receiver's content out block-style, exactly as for a text receiver.

The receiver's block-style content is also what a both-boundary newline authoring looks like
(`<i class="…">⏎\t{b}⏎</i>`), so the dangle must not depend on how that content was written:
authored on one line or on its own lines, the pair lands on the same form. The shedder and the
receiver may each be a component, the receiver may end a run of three (only the pair whose
receiver wraps dangles), and the pair may sit at the root, in a block parent or in an inline
parent. The 100/101 boundary is the receiver's flat opening-tag line: at 100 it fits, nothing
dangles and only the content goes block-style; at 101 the attributes wrap and the `>` dangles.
The last case is a wide shedder whose own line is full: its `>` dangles, and the receiver's
opening line (`><i class="…">`) is 101, so its attributes wrap too.

Two receivers are multiline from an authored line break that is not the dangle's own output: one
whose only line break sits inside its content (`<i class="…">{a}⏎{b}</i>` in the compact
authoring — the boundaries are glued), and one whose opening tag wraps at any width because an
attribute value holds a line break (`title="x⏎y"`). Both still receive the `>`.

Among receivers with content, what keeps the predecessor's `>` intact is *structure*: a receiver
holding a block (`{#if c}x{/if}`) is multiline because of what it contains, not how it was
written, and keeps it (`<b>a</b><i⏎\tclass="…"`) — the control case, which prettier prints
identically.

- `unformatted_ours_compact.svelte` — every case on one line but for the two authored line
  breaks above. tsv normalizes it to `input.svelte`; prettier to
  `prettier_variant_hugged.svelte`.
- `unformatted_ours_multiline.svelte` — each receiver's content on its own lines with its
  opening tag flat (`</b><i class="…">⏎\t{b}⏎</i>`). tsv normalizes it to `input.svelte`;
  prettier to `output_prettier.svelte`.
- `prettier_variant_hugged.svelte` — prettier's form of the compact authoring: it keeps the
  receiver's attributes on the pair's line and dangles the receiver's own delimiters
  (`<i class="…"⏎\t>{b}</i⏎>`). Prettier-stable; tsv normalizes it to `input.svelte`.
- `output_prettier.svelte` — prettier's form of `input.svelte`: `</b><i` glued, the receiver's
  attributes wrapped under it. In the wide-shedder case and the block-content control it agrees
  with tsv.

The `>` moves only inside an end tag, so every form parses to a byte-identical AST —
render-safe.

## Reason

Design choice. An opening tag never sits at a line end after a space, and the G2 dangle is how
a glued pair honors that. What non-block content the receiver holds does not change the pair's
layout, and the receiver's content lines are the layout's own output, so the dangle has to be a
one-pass fixed point whatever that content. See
[conformance_prettier_svelte.md §Moving a run: break-before, travel, and welded units](../../../../../docs/conformance_prettier_svelte.md#moving-a-run-break-before-travel-and-welded-units).

## Related

- [elements/inline_break_before_gt_dangle_long](../inline_break_before_gt_dangle_long_prettier_divergence/) — the G2 pair with a text receiver
- [elements/inline_break_before_chain_long](../inline_break_before_chain_long_prettier_divergence/) — G2 over a run of 3+ glued elements
