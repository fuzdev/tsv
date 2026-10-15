# assertion_grouping_exponent

An assertion over a unary expression, as the left operand of `**`: TypeScript reads `(-x as number) ** 2`, and with the assertion blanked the output is `-x ** 2`, which JS rejects (a unary operand of `**` must be parenthesized). Refused (`UnsupportedKind::AssertionGrouping`); the span is the assertion expression. `assertion_grouping_exponent_typeof` and `assertion_grouping_exponent_await` are the same with `typeof` and `await`.

## Override

Both oracles blank the assertion and emit `-x ** 2`, a syntax error, so the refusal is reviewed rather than derived (`override ts-blank-space`, plus the `node` divergence).
