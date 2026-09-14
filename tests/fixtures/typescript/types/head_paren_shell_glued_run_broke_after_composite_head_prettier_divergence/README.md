# head_paren_shell_glued_run_broke_after_composite_head_prettier_divergence

The composite-head face of
[head_paren_shell_glued_run_broke_after_multiline_block_comment](../head_paren_shell_glued_run_broke_after_multiline_block_comment/):
a redundant paren shell holding a run the author GLUED — a multi-line block ahead of a single-line
one — and then broke after, where the shell is not the whole RHS but its leading printed
descendant — a conditional's **check** type (`(⏎/* c1⏎d1 */ /* c2 */⏎B) extends C ? D : E`) and an
intersection's **first** member (`(…⏎G) & H`).

**tsv** (`input.svelte`) strips the shell, the enclosing `=` gap claims the run, and the whole
thing lays out exactly as the shell-free authoring does — the run below the `=`, the head dropping
below the run's closing `*/`:

```ts
type A =
	/* c1
	d1 */ /* c2 */
	B extends C ? D : E;
```

**The two formatters agree on the form; they disagree on how fast they reach it.** Prettier is
**non-idempotent** on the shell authoring at these two seams. Its first pass
(`prettier_intermediate_paren_shell.svelte`) strips the shell, trails the run on the `=` line and
leaves the head flush under `type`, then breaks the conditional (and the intersection) across
lines:

```ts
type A = /* c1
	d1 */ /* c2 */
B extends C
	? D
	: E;
```

Its **second** pass lands on `input.svelte` byte-exact — tsv's one-pass form — and every later pass
holds it. So there is no residual form divergence and no residual indent divergence: what this
directory records is the **normalization** divergence alone (convergence speed), carried by the
`unformatted_ours_paren_shell.svelte` + `prettier_intermediate_paren_shell.svelte` pair, which is
what makes the directory `_prettier_divergence` (rules S8/S11, chain N7).

## Reason

Per Comment Position Philosophy: the run leads the composite's head, so tsv keeps it there and
keeps the author's break after it. The break itself is prettier's own `printLeadingComment` soft
`line`, which the multi-line block forces open — the author's break sits after the single-line
block glued behind it, so it belongs to the RUN, not to one comment, and every separator the author
broke opens with it. A break the shell emitted at its own indent would be one the reparse — which
finds the comment in the `=` gap, the shell gone — re-lays, so the enclosing gap owns the run at
these heads exactly as it does where the shell IS the value. What remains is only the shape
prettier reaches on a second pass and tsv reaches on the first.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(paren shell at a leading EDGE, a forced-break BLOCK run) and
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy).
