# assertion_grouping_precedence

An assertion whose blanking would regroup the expression: TypeScript reads `(b + c as T) / d`, and with the assertion blanked JS reads `b + (c / d)`. Refused (`UnsupportedKind::AssertionGrouping`); the span is the assertion expression.

## Divergence

Node blanks the assertion and emits `b + c / d`, a different program. tsv follows ts-blank-space, which refuses it.
