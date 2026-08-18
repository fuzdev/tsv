# Expression-statement value paren, trailing line comment

A `//` written between an expression statement's value and the `)` that closes
its grouping parens stays **inside** those parens, which are retained for it.

- **tsv**: retains the parens and keeps the comment inside them, on its line.
- **prettier**: strips the parens and defers the comment to end of line, past
  the `)` and the `;`.

The statement's other value positions already answer this way — a declarator
initializer, an assignment RHS and a ternary branch all keep such a comment
inside a retained shell (`const a = (⏎\tb // c⏎);`), through the shared shell
builder. A bare expression statement was the holdout, and one question answered
two ways is what this fixture closes. The pair the statement would print anyway
(`({ a: 1 })`, `(function () {})`) is the **same single pair** — a shell whose
`(` leads the statement already discharges what those parens exist for.

**Why retention rather than prettier's placement.** Deferring a `//` past its
own statement can land it on a line that already holds one, where the two
**merge** into a single comment — the canonical information-losing relocation
[§Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
names as its deciding test. `unformatted_ours_flat.svelte` is that authoring:
prettier turns `(x // c1⏎); // c2` into `x; // c1 // c2`, which reparses as one
comment whose text is ` c1 // c2`, while tsv normalizes it to the retained form
where both survive. From tsv's own fixed point prettier happens to split the
two onto separate lines, so `output_prettier.svelte` shows the relocation
without the merge — the merge is what the authored form reaches, and what the
retention is for.

A trailing **block** comment needs no shell: it does not end its line, so its
placement past the `;` is lossless and stable, and tsv matches prettier there
(the last case).

The same retention holds at the `return`/`throw` operand
([operand_paren_trailing_line_comment](../return_throw/operand_paren_trailing_line_comment_prettier_divergence/)),
at the export value positions
([default_operand_paren_line_comment](../../modules/exports/default_operand_paren_line_comment_prettier_divergence/))
and at the `await`/`yield` operand shell
([redundant_operand_paren_comment](../../expressions/await_yield/redundant_operand_paren_comment_prettier_divergence/)).

Reason: comment preservation. See
[conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation (Expression-statement value paren, line comment).
