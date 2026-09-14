# indexed_access_line_comment_prettier_divergence

A line comment in an indexed access type's `[`→index gap (`A[ // c⏎K]`). tsv
keeps the comment where the author wrote it and drops the index type to the next
line:

```
type I =
	A[// c
	K];
```

**Prettier** relocates the comment out past the access to a statement-trailing
position (`type I = A[K]; // c`).

Per Comment Position Philosophy, tsv preserves the comment's authored position.
Emitting it inline (the previous behavior) let the `//` **swallow** the index
type — non-idempotent content loss; the line comment now forces the break (the
shared `build_trailing_comments_hang_next`).

The second case is the same gap at a run with no `//` in it at all: a **multi-line**
block the author GLUED ahead of a single-line one and then broke after
(`A[/* a⏎b */ /* c */⏎K]`). The multi-line body cannot print flat, so the run's break is
forced — but it is the RUN's break, not either comment's, and the per-comment own-line
rule answers no for both halves (the first is glued, the second single-line):

```
type J = A[/* a
b */ /* c */
	K];
```

Two things had to move for that. The gap's window is now the shared keyword→value one
(`Printer::keyword_value_stripped_paren_hang`) rather than the index's own span start: a
redundant paren **shell** around the index puts its `(` there, so the old window was
EMPTY and the gate saw nothing — the shell then laid its own run out at its own indent,
which the reparse, finding the comment in this gap with the shell gone, re-laid. And the
gate and its emitter both read the run-level question
(`Printer::broke_after_run_holds_multiline_block`) beside the per-comment one, so the
selected hang is actually emitted. `unformatted_ours_paren_shell.svelte` is the shelled
spelling of both cases; tsv lands it on `input` in one pass.

The `=` keeps its hug on this case where the `//` above breaks it, and it rides the alias
gate's own per-comment reading deliberately: `build_type_alias_eq_value_doc`'s hug is
`comment_driven_break && !comments_force_own_line_between(value_span)` — the value bears a
comment that lays it out, and no comment in it forces a line of its own. For this run
(`A[/* a⏎b */ /* c */⏎K]`, a bare `TypeReference` index with no directive) that reading
answers no, so the hug holds and the break stays inside the brackets the indexed access
owns — which is the answer an indexed access already gets whenever it breaks internally.
A `//` in the value answers yes there and forces an own-line break the alias reads for
itself. Both authorings — bare and shelled — converge on the hug in one pass, so the
per-comment reading is the right one to leave at this gate.

**Prettier has no usable answer at that case**: it relocates the run across the `[`
(`type J = A /* a⏎b */ /* c */[K]`), putting a line break in the `[no LineTerminator
here]` gap before the index suffix — and its OWN next pass then reads that as a
**different program**, `type J = A;` plus an `[K];` expression statement. That second
pass is pinned by `audit_signature.txt`; see
[conformance_prettier.md §Prettier bug index](../../../../../docs/conformance_prettier.md#prettier-bug-index).

The neighbouring object→`[` gap (`A // c⏎[K]`) has no counterpart fixture because
the shape does not exist: a type's index suffix may not follow a line break, so
both parsers read `A⏎[K]` as two statements (`type X = A;` plus an
`ArrayExpression`), and a `//` there forces exactly that break. That gap can hold
only a single-line block (`A /* c */[K]`), which stays glued.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
