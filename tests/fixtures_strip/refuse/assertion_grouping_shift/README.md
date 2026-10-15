# assertion_grouping_shift

An assertion over a comparison, followed by a shift: TypeScript reads `(b < c as T) >> d`, and with the assertion blanked JS reads `b < (c >> d)`, a different program. Refused (`UnsupportedKind::AssertionGrouping`); the span is the assertion expression.

## Override

ts-blank-space reads the operator after the assertion with TypeScript's scanner, which scans `>>` as a lone `>` (a comparison, as tight as the `<`), so it blanks the assertion and emits `b < c >> d`. The refusal is reviewed rather than derived (`override ts-blank-space`, plus the `node` divergence: Node blanks it too).
