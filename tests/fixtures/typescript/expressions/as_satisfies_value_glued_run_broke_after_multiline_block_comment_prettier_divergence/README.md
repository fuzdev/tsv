# as_satisfies_value_glued_run_broke_after_multiline_block_comment_prettier_divergence

A leading run in an `as` / `satisfies` cast's keyword→type gap that the author GLUED — a
multi-line block ahead of a single-line one — and then broke after
(`x as /* c1⏎d1 */ /* c2 */⏎B`).

**tsv** (`input.svelte`) keeps the whole run where it was written, trailing the keyword, and
hangs the type one level under it — the indent a `//` in the same gap takes, and the one a lone
multi-line block the author broke after (`x as /* c5⏎d5 */⏎F`, the fixture's control) already
took:

```ts
const a = x as /* c1
	d1 */ /* c2 */
	B;
```

**Prettier** (`output_prettier.svelte`) relocates the run's tail across the keyword, binding
`/* c2 */` to the operand instead of to the type, and pulls the type back up onto the multi-line
block's closing line:

```ts
const a = x /* c2 */ as /* c1
	d1 */ B;
```

That form is prettier's own fixed point — it is idempotent here — so there is no chain to pin.

⚠️ With a `//` TAIL on the run (`x as /* c6⏎d6 */ // c7⏎H`) prettier does worse than relocate: it
WELDS the line comment into the multi-line block's first line (`x as /* c6 // c7⏎d6 */ H`), so
`// c7` stops being a comment and becomes text of the block — a content loss its own next pass
can no longer see (the welded form is a fixed point). It is specific to `as` / `satisfies`: at
`keyof`, `await`, `new`, a `case` test and `export default` the same run keeps the `//` separate.
See [conformance_prettier.md §Prettier bug index](../../../../../docs/conformance_prettier.md#prettier-bug-index).

## Reason

Per Comment Position Philosophy: the author wrote both comments after the keyword, leading the
type, so tsv keeps them there rather than splitting the run across the keyword. The break after
the run is prettier's own `printLeadingComment` soft `line`, which the multi-line block forces
open; whether the block itself or a single-line block glued behind it carries the author's break
is a property of the RUN, not of one comment, so every separator the author broke opens with it.
The continuation indent is the one every forced keyword→value break takes.

The lone-block spelling of the same gap is
[as_satisfies_value_own_line_block_comment](../as_satisfies_value_own_line_block_comment_prettier_divergence/);
the type-position twin of this authoring is
[head_paren_shell_glued_run_broke_after_multiline_block_comment](../../types/head_paren_shell_glued_run_broke_after_multiline_block_comment/),
whose shelled variant (`unformatted_ours_paren_shell.svelte` here) strips to the same fixed point.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(keyword→value gaps, a forced break after a block comment) and
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy).
