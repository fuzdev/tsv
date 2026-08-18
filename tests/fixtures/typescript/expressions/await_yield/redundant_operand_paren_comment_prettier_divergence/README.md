# Await/yield operand comment, redundant parens

The redundant-paren twin of
[grouped_operand_comment](../grouped_operand_comment_prettier_divergence/),
which pinned only the case where the parens are *required* (`await (x + y /* c
*/)`). The rule holds at every `await`/`yield` operand: a comment written
between the operand and the `)` that closes its grouping parens stays **inside
those parens**, and the shell is retained when the parens were otherwise
redundant.

- **tsv**: retains the shell and keeps the comment inside it.
- **prettier**: strips the redundant parens and relocates the comment past the
  `)` — then, on a second pass, past the `;` as well.

**Both of prettier's relocations lose something, which is why tsv diverges
rather than matching.**

A **block** comment leaves prettier without a fixed point in one pass: `await (x
/* c */)` prints `await x /* c */;`, which is not stable — the reparsed
`AwaitExpression` no longer spans the `)`, so the comment falls to the
statement's terminator gap and pass 2 emits `await x; /* c */` (pinned in
`audit_signature.txt`). Adopting pass 1 would put tsv's own **Core Invariant**
— an input formats to itself — out of reach for this authoring, since the form
tsv settled on would not survive its own reparse.

A **line** comment carried past its own statement lands on a line that may
already hold one, and the two **merge**: `await (z // c1⏎); // c2` becomes
`await z; // c1 // c2`, which reparses as a *single* comment whose text is
` c1 // c2` — the second comment stops existing. That is the canonical
information-losing relocation
[§Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
names as its deciding test, and the comment census measures it directly
(`MISSING "c1"` · `MISSING "c2"` · `EXTRA "c1 // c2"`).

`yield` reaches this fixture where `await` does not: `yield` binds looser than
`+`, so even a binary operand's parens are redundant there (`yield (a + b /* c
*/)`), while `await` binds tighter and keeps them ([grouped_operand_comment](../grouped_operand_comment_prettier_divergence/)).

Retention is the comment's doing — a redundant paren pair with nothing in the
gap still strips (`await (x)` → `await x`), which the `unformatted_ours_*`
variants pin. The same rule already holds at the unary operators (`!(y /* c
*/)`, `typeof (y /* c */)`), at the non-null operand
([operand_line_comment](../../non_null/operand_line_comment_prettier_divergence/))
and at the angle-bracket type assertion — `await`/`yield` were the two holdouts.

Reason: comment preservation. See
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation (Await/yield operand, redundant parens).
