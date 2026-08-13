# fill_tail_move_after_break_long_prettier_divergence

The moving-tail half of the boundary
[inline_wide_element_content_tail_long](../inline_wide_element_content_tail_long_prettier_divergence/)
pins hugging: the same per-width tail boundary, taken one column past the point where the run's
first word still fits after its predecessor. There the run cannot hug, so it **moves whole** to a
fresh line and the boundary space is spent on that break rather than re-emitted beside it — a
re-emitted space stands at the head of the continuation line, which is neither the author's nor a
break the next parse reads back the same way.

The predecessor here carries a **forced** break — an element whose content is authored across
lines (and does not fit inline either way), and an expression tag whose expression must break.
That is one boundary rule with one answer, so the fixture varies the two axes it could have been
keyed on and holds the answer fixed across both: the **kind** of predecessor (element, tag) and
the **position** of the run (non-terminal, terminal).

## Cases

- **element, 100 / 101** — the first word hugs the intact closing tag at exactly print width;
  one character wider, the run starts a fresh line unled by a space and packs there.
- **element, 100 / 101, terminal** — the identical pair with nothing following the run.
- **tag, 100 / 101** — a forced-break expression tag reaches the same two answers at its own
  column.
- **tag, 101, terminal** — the control on the position axis: this position already took the
  run's own fill `line`, so it is what the non-terminal case must agree with.
- **`svelte:component`, 101** — the same answer through the `svelte:*` printer. A component is
  the same `Element` node as the first cases and needs no pin of its own, but a `svelte:*`
  element is a distinct node with a distinct builder, and that builder has twice drifted from
  the shared element pipeline — so this case is a drift guard, not a second rule.

`unformatted_ours_compact` authors every element case on one line; tsv normalizes each to `input`
in one pass. Prettier holds a stable form per authoring — `output_prettier.svelte` is its form
from `input` — and tsv normalizes those to `input` too.

⚠️ Only the element cases carry an F1 claim. A forced-break **tag** breaks identically under every
authoring, so a stray leading space there is its own fixed point: idempotency, the fuzzer and the
round-trip are all blind to it and only the column separates the two forms.

## Reason

Design choice, the same one the rest of this family records. The tail boundary after a multiline
predecessor is a per-width fill decision measured from that predecessor's own end column, however
it came to be multiline, and a run that cannot hug spends its boundary space on the break — so
every render-free authoring of the document reaches one fixed point. Prettier instead groups that
boundary with the element, so a multiline element always drops a non-terminal tail while a
terminal one hugs and the line runs past print width. The boundaries tsv moves are render-free
under Svelte 5.
See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
