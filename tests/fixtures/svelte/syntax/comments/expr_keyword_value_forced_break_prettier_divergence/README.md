# expr_keyword_value_forced_break_prettier_divergence

The keyword→value forced break inside Svelte template expressions: a block comment trailing
`await` / `new` that the author broke after, then a **multi-line** comment glued to the value
(`{await /* x */⏎/* y⏎*/ a}`). The break after `/* x */` is forced, and the template islands take
the answer `<script>` gives
([await_new_operand_leading_run_multiline_tail](../../../../typescript/expressions/await_new_operand_leading_run_multiline_tail_prettier_divergence/)).

tsv (`input.svelte`) hangs the rest of the run one level under the keyword, inside the braces:

```svelte
{await /* x */
	/* y
	 */ a}
```

Prettier leaves it flush (`output_prettier.svelte`):

```svelte
{await /* x */
/* y
 */ a}
```

The cases cover the `{expr}` tag, `{@html}`, a braced attribute value, `{@const}`, and a block
head. In the block head prettier welds the run onto the keyword's line instead
(`{#if a && new /* x */ /* y⏎ */ C()}`), since it never wraps a block head; tsv's head wraps, its
`}` dangling to base as at every wrapped block head
([§Svelte: Blocks](../../../../../../docs/conformance_prettier_svelte.md#svelte-blocks)).

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(keyword→value gaps, a forced break after a block comment) and
[conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).
