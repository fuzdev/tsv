# directive_gap_text_prettier_divergence

The break in front of a format-ignored node is the **authored gap, emitted once**. When the
frozen node is a **text**, the parser folds that gap into the node itself, so the gap is
already inside the frozen slice — printing a structural break on top of it emits the same
boundary twice, and each pass folds the printer's break into the next pass's slice, so the
gap grows by a line forever.

Prettier has the same defect and **never converges** here (`prettier_nonconvergent.txt`),
so there is no oracle to compare against; tsv's claim is the one every fixture makes
anyway — `input.svelte` formats to itself.

## Reason

Content preservation and F1. Two rules settle it, and each is one the printer already
states elsewhere:

- **The gap is printed once.** A frozen slice that already opens with a line break carries
  the boundary itself; a slice glued to the directive carries none and takes the printer's.
  The whitespace-only spelling of the same gap — the run beside a frozen **element** — is
  the half the printer skips and re-emits, and it keeps its authored blank
  ([directive_gap_blank](../directive_gap_blank/)).
- **The trailing run goes wherever the boundary after it is the printer's.** That is two
  places, and they are the two where the printer emits a line whatever precedes: nothing
  follows, so the fragment's own close is the boundary; or the follower **owns its line** (a
  `{@const}` / `{#snippet}` — the same seam `handle_content_text_child` answers with
  `next_owns_line`, blank and all). Every other follower reads the previous text's trailing
  whitespace before deciding, so it prints nothing beside a slice that already ends in a
  break and the run has to stay — the fourth case pins that side. A whitespace-collapsing
  container (`<select>`, `<datalist>`, …) separates every child itself, so there the
  boundary is always the printer's.

The frozen bytes are the author's, so the gap keeps the exact indentation it was written
with — the un-indented `text1` in three of these cases is the authored form, not a layout
tsv chose. That is the same reading prettier gives those bytes.

See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).

## Files

- `unformatted_ours_hugged.svelte` — every document authored hugged. Each grew a line per
  pass while the gap was printed twice; tsv reaches `input.svelte` in one pass.
- `prettier_nonconvergent.txt` — prettier grows the gap on every pass, so no
  `output_prettier.svelte` exists.

## Related

- [directive_gap_blank](../directive_gap_blank/) — the whitespace-only spelling of the same gap
- [blocks/body_blank_break](../../../blocks/body_blank_break_prettier_divergence/) — the rule
  that opens a body on an authored blank, which is what reaches this gap
