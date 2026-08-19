# update_prefix_operand_paren_comment_prettier_divergence

A comment between a **prefix** `++`/`--`'s operand and the `)` closing its parens
(`++(a as T /* c1 */)`).

**tsv**: keeps the comment inside the parens, where the author wrote it — retaining
a shell that would otherwise be redundant (`++(b /* c2 */)`), and expanding it
around a `//`, which cannot trail before the `)`.

**Prettier**: relocates the comment outside the parens (`++(a as T) /* c1 */;`),
and is **not idempotent** there — its own second pass moves every one of them again,
past the `;` (`++(a as T); /* c1 */`, pinned in `audit_signature.txt`).

## Reason

This is the `await` / `yield` rule at a third construct, reached for the same
reason: a prefix update **spans its own `)`**, so the enclosing terminator gap
begins past it and nothing outside can see in. Two things follow.

Unclaimed, the comment is not relocated but **DROPPED** — `++(a as T /* c1 */);`
printed `++(a as T);` and `++(⏎d⏎// c4⏎);` printed `++d;`, the comment simply gone.
The postfix twin has no such hole: its span ends past the operator, so the region
before it is inside the node and
[update_postfix_paren_line_comment](../update_postfix_paren_line_comment_prettier_divergence/)
owns it.

And relocating it outside has no fixed point, which is what makes keeping it a
correctness choice rather than a preference: on reparse the comment lands in the
statement's terminator gap and moves again, past the `;` — prettier's two passes
above are that transient. A `//` is worse than a transient: carried past the `;` it
merges with whatever already trails that line, and the second comment stops being
one (see
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)).

The **`(`→operand** gap is a different question with a different owner — the
operator→operand emitter, which strips a redundant shell and pulls the comment up
(`++(/* c */ a)` → `++/* c */ a`, matching prettier). `c5`/`c6` pin the two
together: the leading comment leaves the parens, the trailing one keeps them.

A comment-free operand is the control — a required pair stays (`++(f as T)`), a
redundant one is stripped (`++g`) — so the retention above is the comment's doing.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
