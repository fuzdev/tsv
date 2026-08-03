# case_before_colon_line_comment_prettier_divergence

A line comment in a switch case's head→`:` gap (`case x // c⏎:`,
`default // c2⏎:`). tsv keeps the comment after the head and drops the `:` to
a continuation line **indented one level** (uniform forced-continuation
indent); the `//` runs to end-of-line, so the `:` cannot stay on it — emitting
the gap inline and appending `:` would **swallow the colon into the comment**
(`case x // c:`), which does not reparse. Content preservation, not a layout
choice — the same argument as the switch `)`→`{` gap
([head_body_line_comment](../head_body_line_comment_prettier_divergence/)).

Prettier has **two destinations from this one gap, keyed on the authoring**:

- From the same-line authoring (input) it relocates the comment **across the
  `:`**, trailing the label line (`case x: // c` — `output_prettier.svelte`).
  A **run** it splits across the boundary — first comment trailing the label,
  the rest re-bound into the body leading the first consequent statement
  (`case y: // c3⏎\t\t\t// c4⏎\t\t\tb();`) — so the run's comments end up in
  two different syntactic positions; tsv keeps the run in place, in order,
  each comment distinct above the continuation `:`.
- From the **own-line** authoring it moves the whole run into the body
  (`case x:⏎\t\t\t// c⏎\t\t\tb();` — `variant_own_line.svelte`, one pass,
  dual-stable); tsv pulls the comment up to trail the head and normalizes to
  the same continuation form — own-line-ness is authoring signal for a leading
  position, not a trailing one.

```ts
// tsv (preserve + continuation indent)   // prettier (trail past `:`)
switch (a) {                              switch (a) {
	case x // c                           	case x: // c
		:                                 		b();
		b();                              }
}
```

- `unformatted_ours_spaces.svelte` — the flush authoring (`:` at case indent):
  tsv normalizes it to input.
- `unformatted_ours_own_line.svelte` — the own-line authoring: tsv normalizes
  it to input; prettier takes it into the body instead.
- `variant_own_line.svelte` — prettier's own-line landing, dual-stable.

The own-line **block** sibling (which collapses inline — a block forces no
break) is
[case_before_colon_own_line_block_comment](../case_before_colon_own_line_block_comment_prettier_divergence/);
the after-`:` and inline-block gaps are
[case_colon_comment](../case_colon_comment_prettier_divergence/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation and §Uniform Forced-Continuation Indent.
