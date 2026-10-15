# assertion_grouping_exponent_await

`assertion_grouping_exponent` with `await`: the blanked `await x as number ** 2` reads `await x ** 2`, a syntax error. Refused (`UnsupportedKind::AssertionGrouping`); the span is the assertion expression.

## Override

Both oracles blank the assertion and emit `await x ** 2`, so the refusal is reviewed rather than derived (`override ts-blank-space`, plus the `node` divergence).
