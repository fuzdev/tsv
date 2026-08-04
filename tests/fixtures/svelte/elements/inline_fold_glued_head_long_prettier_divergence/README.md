# inline_fold_glued_head_long_prettier_divergence

A **welded run** — inline element, glued text, inline element — followed by terminal trailing
text, so the run's last element is the after-element fold's head. Both halves of the glued rule
meet here:

- **Unbreakable inside.** No break may land in either glued boundary; doing so injects a rendered
  space (`.x.wyyyyyyyy` → `.x.w yyyyyyyy`).
- **But not immovable.** The run still has the whitespace boundary in *front* of it, which is
  ordinary inter-node whitespace the compiler collapses to one space. tsv spends it: the whole run
  travels to a fresh line **together**, where it fits.

Getting only the first half gives the wrong layout — the run stands on the text line and tears its
last element open block-style, spending a break that buys nothing while leaving the run stranded.

Cases (in order):

- **whole line @100** — everything fits, all inline (control).
- **line @100** — the run still fits after its preceding word, so only the terminal ` tail` drops.
  Prettier keeps the whole 105-char line
  (◆[print_width](../../../../../docs/conformance_prettier.md#print-width-philosophy)).
- **@101** — the run no longer fits, so it travels whole to a fresh line: `<code>.x</code>.w<b>…</b> tail`.
- **spaced control @101** — same widths and same column, but the boundary sits *inside* the run.
  There the break is available in the interior, so only the part after it moves and `<code>.x</code>.w`
  stays put. The contrast is the assertion: identical widths, only the boundary's location differs.

tsv: the run travels to the boundary in front of it, intact. Prettier keeps it on the text line and
dangles the closing `>` (`</b⏎> tail`) — see `prettier_variant_dangle.svelte` (prettier's stable
form, which tsv normalizes to `input.svelte`). `unformatted_ours_compact.svelte` is the compact
one-line authoring both formatters start from: tsv normalizes it to `input.svelte`, prettier to the
dangle form. `input.svelte` is itself prettier-stable, so there is no `output_prettier.svelte`.

Every boundary tsv moves here is render-free — inter-node whitespace collapses to one space — so the
output renders identically to the input (confirmed by `render_compare`).

⚠️ Nothing but the render oracle can catch a regression in the first half: a break that splits a
glued boundary produces output that is its own fixed point, so F1, the seeded fuzzer,
`authoring_audit` and the round-trip all pass straight through it.

## Reason

Design choice: a glued boundary is never split (its presence is render-significant under Svelte 5),
but the welded unit still spends the render-free boundary in front of it rather than standing and
overrunning.
See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
