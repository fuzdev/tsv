# inline_welded_run_after_sibling_long_prettier_divergence

The travel rule from the other side of the boundary. Every case in
[inline_welded_run_travel_long](../inline_welded_run_travel_long_prettier_divergence/) varies what
the run is made of, what ends it and what follows it, and each has **text** in front of the run —
so the whitespace boundary the run travels to belongs to that preceding text. Here it does not:
the previous sibling is an inline **element** or **tag**, so the boundary is the run's *own*
leading whitespace. The measurement is the same whole-flat one, so the run reaches the same form.

The cases vary what heads the run and how badly it overflows, and all reach it:

- **lone-word head @100** — the run still fits after the sibling and stays on its line (control).
  Its head is a single glued word (`(`) carrying no whitespace of its own.
- **lone-word head @101** — the run travels whole. The `(` head and the `):` tail stay glued to
  the element, so the boundary that breaks is the one in front of the entire unit.
- **lone-word head @101, expression tag sibling** — which kind of inline sibling precedes the run
  cannot change where it breaks. Same width as the element case, same form.
- **two-word head @101** — the head is `zz (` rather than `(`, so the run is preceded by a word
  that *does* own a whitespace boundary. The break still lands in front of the unit, leaving the
  head's earlier word packed. How many words head the run cannot change the answer.
- **far too wide to pack** — the overflow is much more than the one column a dangled `>` buys, yet
  the run still fits on its own line. It travels there whole.
- **element wider than a whole line** — travelling cannot make it fit, and it travels anyway
  rather than standing: the run takes the fresh line and the element then lays out block-style
  there, both tags intact.

## Prettier's forms

Prettier's answer here is both authoring- and shape-dependent, which is why this fixture carries
two prettier files:

- `output_prettier.svelte` — prettier from **this** authoring. It keeps tsv's form on five of the
  six, and dangles only the two-word-head case (`</a⏎>):`), whose traveled form it will not hold
  from either authoring.
- `prettier_variant_dangle.svelte` — prettier from the compact one-line authoring
  (`unformatted_ours_compact.svelte`), its other stable form, which tsv normalizes to
  `input.svelte`. Here every case that must break dangles, in three spellings: the closing `>`
  alone, an attribute wrap plus `">qqqq</a⏎>`, and an attribute wrap plus a dangled `⏎>qqqq</a⏎>`.

So prettier's dangle regime is bounded by what one column buys: at exactly 101 a dangled `>` makes
the line fit and prettier takes it, and once the overflow is wider than that it breaks at the same
boundary tsv does. tsv never dangles at either width.

The boundary tsv spends is inter-node whitespace, and the glued boundaries inside the run are
never touched, so the output renders identically to the input.

⚠️ A regression here is invisible to every idempotency-shaped gate: the dangled and torn-open
forms are each their own fixed point, so F1, the seeded fuzzer, `authoring_audit` and the
round-trip all pass through them, and neither is over-width, so nothing measures a column either.
`input.svelte` failing to be a tsv fixed point is what catches it.

## Reason

Design choice: "unbreakable inside" is not "immovable" — the unit spends the render-free boundary
in front of it rather than standing and overrunning, and tsv lays wrapping inline content out
block-style where prettier dangles the tag delimiters.
See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
