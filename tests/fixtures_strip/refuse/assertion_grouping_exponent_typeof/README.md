# assertion_grouping_exponent_typeof

`assertion_grouping_exponent` with `typeof`: the blanked `typeof x as any ** 2` reads `typeof x ** 2`, a syntax error. Refused (`UnsupportedKind::AssertionGrouping`); the span is the assertion expression.

## Override

Both oracles blank the assertion and emit `typeof x ** 2`, so the refusal is reviewed rather than derived (`override ts-blank-space`, plus the `node` divergence).
