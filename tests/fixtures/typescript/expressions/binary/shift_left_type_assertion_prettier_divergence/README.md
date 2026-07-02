# shift_left_type_assertion_prettier_divergence

A `<<` token splitting at the start of an **angle-bracket type assertion**
(`<<T>() => R>x`, i.e. `<(<T>() => R)>x`) is valid by tsv's parse oracle —
acorn-typescript splits the shift token and reads a generic function type as
the asserted type — and tsv keeps it stable. A statement cannot begin with a
left-shift operand-less `<<`, so the split is unambiguous.

Prettier's `typescript` parser (tsc) **rejects** it:

```
Expression expected.
```

tsc never splits `<<` at the assertion position (the sibling call/new/type
positions in [shift_left_vs_type_args](../shift_left_vs_type_args/) are
prettier-formattable and covered there; the other rejected positions are
[shift_left_class_extends](../shift_left_class_extends_prettier_divergence/)
and [shift_left_typeof_query](../shift_left_typeof_query_prettier_divergence/)).
`prettier_rejects.txt` pins the error message; rule F6 live-verifies that
prettier still rejects the input, failing loudly if tsc gains the split or the
error morphs.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Prettier rejects valid input.
