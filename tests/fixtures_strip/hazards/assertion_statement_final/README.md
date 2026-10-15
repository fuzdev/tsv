# assertion_statement_final

A statement-final `as` / `satisfies` with no `;` after it blanks to a `;` written right after the expression, so the next line cannot continue it (`b(c)`, `e[f]`). The rule is per statement, not per hazard: the last statement, followed by nothing, gets one too.

## Divergence

Node writes the `;` one column later (at the `as` keyword rather than after the expression), and writes none after the last statement. The same program; tsv follows ts-blank-space's bytes.
