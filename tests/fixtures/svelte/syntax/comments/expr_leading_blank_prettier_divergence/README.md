# expr_leading_blank_prettier_divergence

An author **blank line** after a line comment in a braced head's head→value gap, in both
authorings of the sweeps [expr_leading_line](./../expr_leading_line_prettier_divergence/)
(trailing the head) and [expr_leading_own_line](./../expr_leading_own_line_prettier_divergence/)
(on its own line). The `//` forces the break the blank sits on, so the blank survives — in
both formatters. The divergence is the two siblings' and nothing more: the continuation's
indent, and the own-line comment's placement.

tsv:

```svelte
{@html
	// c2

	expr
}
```

Prettier: `{@html // c2⏎⏎expr}` — the comment pulled up, the blank kept, the value flush.

## Reason

The blank is the author's, and the break it separates is forced by the comment, so it is
kept at every forced continuation — the rule
[conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)
states for the keyword→value gaps, at their Svelte face. The run's placement is
[§Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)'s,
as in the own-line sibling.

What the cases pin — every head in both authorings, the odd-numbered comment on its own
line and the even-numbered one at the end of the opener:

- **c1 / c2** — the `{expr}` tag; **c3 / c4** — `{@html}`; **c5 / c6** — `{@render}`;
  **c7 / c8** — an `{#if}` head; **c9 / c10** — an `{#each}` head; **c12 / c13** — a
  spread; **c15 / c16** — an attribute value. The blank after the comment survives, the
  value below it.
- **c11** — the `{@const}` init, and **c14** — a `bind:` value: the two hosts already in the
  own-line shape (the break-after-operator layout and the block wrap), where a comment on
  the opener's line is pushed down to its own line by both formatters
  ([expr_leading_line](./../expr_leading_line_prettier_divergence/)), so only the own-line
  authoring exists to pin.
- **c17 / c18** and **c19 / c20** — a blank **inside** a run keeps its place between the
  two comments, from either authoring.
- **c21** — `{@debug}`, whose own emitter keeps the blank the same way; prettier strips the
  comment outright there.
- **c22** — a `bind:` sequence's comma gap, and **c23 / c24** — a run there with the blank
  inside it: the sequence's own comma-gap emitter keeps the blank as every head does (prettier
  keeps it too).

## Related

- [expr_leading_own_line](./../expr_leading_own_line_prettier_divergence/) — the own-line sweep without the blank
- [expr_leading_line](./../expr_leading_line_prettier_divergence/) — the trailing sweep without the blank
- [case_test_gap_own_line_line_comment](../../../../typescript/statements/switch/case_test_gap_own_line_line_comment_prettier_divergence/) — the same blank kept at a TypeScript keyword→value gap (its `c7`)
