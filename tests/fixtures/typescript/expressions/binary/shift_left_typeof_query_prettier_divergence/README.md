# shift_left_typeof_query_prettier_divergence

A `<<` token splitting into the **instantiation type arguments of a `typeof`
query** (`type Y = typeof f<<T>() => void>`) is valid by tsv's parse oracle —
acorn-typescript splits the shift token and reads a generic function type
argument — and tsv keeps it stable. A type position has no shift operator, so
the split is unambiguous.

Prettier's `typescript` parser (tsc) **rejects** it:

```
';' expected.
```

tsc never splits `<<` in a type-query's type-argument list (the sibling
call/new/type-reference positions in
[shift_left_vs_type_args](../shift_left_vs_type_args/) are
prettier-formattable and covered there; the other rejected positions are
[shift_left_class_extends](../shift_left_class_extends_prettier_divergence/)
and
[shift_left_type_assertion](../shift_left_type_assertion_prettier_divergence/)).
`prettier_rejects.txt` pins the error message; rule F6 live-verifies that
prettier still rejects the input, failing loudly if tsc gains the split or the
error morphs.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Prettier rejects valid input.
