# less_than_keyword_member_prettier_divergence

A type-operator keyword (`keyof`, `readonly`, `infer`, `unique`) followed by a
`.` member access after `<` (`p < keyof.a > (t, u)`) is valid by tsv's parse
oracle — acorn-typescript reads a comparison on the member expression
`keyof.a`, since an operator keyword cannot head a qualified type name (unlike
an atom keyword: `p < string.length > (t, u)` IS an instantiation over the
entity name `string.length`) — and tsv keeps it stable.

Prettier's `typescript` parser (tsc) **rejects** it:

```
Type expected.
```

tsc commits to the type-argument reading at the operator keyword and then has
no type to parse after the `.`. `prettier_rejects.txt` pins the error message;
rule F6 live-verifies that prettier still rejects the input, failing loudly if
tsc gains the backtrack or the error morphs. The prettier-formattable rows of
the same follow-token family live in
[less_than_keyword_follow](../less_than_keyword_follow/).

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md)
§Prettier rejects valid input.
