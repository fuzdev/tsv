# paren_binary_base_long_prettier_divergence

A parenthesized binary expression used as a member-access base, long enough that the parens must
break onto their own lines, whose left operand is itself a parenthesized binary.

tsv: breaks the parens and holds the operand chain together while it fits (`(\n\t(a && b) || c\n).toString()`)
Prettier: breaks the parens **and** splits the operand chain onto separate lines

| Case                                    | tsv                | Prettier             |
| --------------------------------------- | ------------------ | -------------------- |
| `((a && b) \|\| c).toString()` at 100   | inline             | inline               |
| `((a && b) \|\| c).toString()` at 101   | operand chain flat | one operand per line |
| `((a + b) / c).toString()`              | operand chain flat | one operand per line |
| `((a && b) ?? c).toString()`            | operand chain flat | one operand per line |
| `((a && b) \|\| c)!.toString()`         | parens hang        | welds `)!.toString()` onto the last operand |
| `((a && b) \|\| c)?.toString()`         | operand chain flat | one operand per line |
| `(a && b && c).toString()` (flat chain) | one operand per line | one operand per line |

## Reason

Design choice, and minimal breaking. Breaking the parens is what the width demands; splitting the
operand chain as well is a second break the width does not ask for. tsv takes only the break it
needs, so the chain still reads as one expression.

The rule is uniform along both axes Prettier varies on:

- **Operator family** — arithmetic, logical (`&&` / `||`), and nullish (`??`) bases take the
  identical shape. The alternative — a third layout for logical and nullish bases that welds the
  closing `).member` onto the last operand (`((a && b) ||\n\tc).toString()`) — matches neither tsv's
  own arithmetic shape nor Prettier's, so it is a shape with no constituency.
- **What follows the base** — a plain `.member`, a non-null `!.member`, and an optional `?.member`
  all lay out identically. Prettier varies: it hangs the parens for `.` and `?.` but welds
  `)!.member` onto the last operand. This is the same stance as
  `non_null_paren_base_long_prettier_divergence`, extended from call/await bases to binary ones.

A **computed** lookup (`(…)[k]`) is deliberately not covered here: it is governed by the separate
never-break-before-`[` rule (`member/computed_paren_base_long/`), which breaks the brackets before
the parens ever hang, so it would pin two rules' interaction rather than this one.

A **flat** chain (`a && b && c`, no parenthesized operand) is not part of the divergence — there is
no nested operand to hold together, so both formatters break every operand. It is pinned here as the
boundary of the rule.

## Related

- `member/paren_base_trailing_long/` — a parenthesized **call** or `as`-cast base, where tsv matches Prettier.
- `member/non_null_paren_base_long_prettier_divergence/` — the same "layout is independent of `!`" stance for call/await bases.
- `typescript_specific/non_null/long/` — the other non-null layouts, none of which diverge.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §TypeScript (Parenthesized binary member base) and [§Print Width Philosophy](../../../../../../docs/conformance_prettier.md#print-width-philosophy).
