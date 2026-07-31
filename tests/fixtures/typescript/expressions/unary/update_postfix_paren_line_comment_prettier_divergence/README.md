# update_postfix_paren_line_comment_prettier_divergence

A line comment (or multiline block) between a postfix `++`/`--`'s operand and the
operator (`(a // c⏎)++`).

**tsv**: keeps the grouping parens that hold the comment, so it stays exactly where
the author wrote it:

```
(
	a // c1
)++;
```

**Prettier**: floats the comment out past the whole statement:

```
a++; // c1
```

## Reason

The same ASI-sensitive gap as the
[`as`/`satisfies` cast operand](../../as_satisfies_operand_line_comment_prettier_divergence/),
one construct over: a line break before a postfix operator ends the expression, so
`a // c⏎++` parses as `a;` then `++;` and never as an update. A comment that
occupies more than one line can therefore only ever have reached this gap from
*inside* a grouping paren shell, and the shell has to survive for the comment to
keep its place — the same shell that a cast operand (`(c as T // c4⏎)++`) needs
anyway, so no second pair is added.

Left inline this is **content loss**: `(a // c⏎)++;` formatted to `a // c++;`,
pulling the `++;` code into the comment. That output still parses (as a bare `a`
statement), so it was a fixed point and idempotency and round-trip were both blind
to it.

The **multiline block** case is worse than a swallow in both formatters, and tsv's
old output and prettier's current one are the same shape: `d /* m1⏎m2 */++;` puts a
real line break before the operator, so it **does not reparse** — a prettier bug
(see [conformance_prettier.md §Prettier bug index](../../../../../../docs/conformance_prettier.md#prettier-bug-index)),
and the reason the gate here is "spans lines", not "is a line comment".

A **single-line block** comment forces nothing and stays inline without parens
(`f /* c7 */++`, matching prettier) — it is pinned here as the control, alongside
the regular [update_operator_comment](../update_operator_comment/) fixture, which
covers the block-comment cases on both sides.

See
[conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).

The retained shell **expands** — the operand takes its own indented line rather
than gluing to the `(` — for every comment that makes the shell span lines, so a
`//` and a multiline block reach one form at this gap instead of two.

The shell has **two** gaps, and the other one keeps it for a different reason:
nothing else emits the `(`→operand gap. A comment there that is neither glued to
the operand (which would make it `owned_by_node`, printed from the operand's own
doc) nor inside the operand's span belongs to no node at all, so stripping the
shell **dropped** it outright — `( // c⏎x) as A` formatted to `x as A`, the
comment simply gone, and the census is what saw it. Retaining on either gap is one
rule for one shell rather than two half-rules that disagree about `( // b⏎x // c⏎)`
(where the trailing comment survived and the leading one did not). A `//` on the
`(` line stays on it (`( // c12`); every other leading comment takes the ordinary
run, so an own-line block keeps its own line and a glued one leads the operand
inline.
