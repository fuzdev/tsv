# property_definite_no_init_prettier_divergence

A class property with a bare definite-assignment assertion — `b!;`, no type
annotation, no initializer — is a TypeScript early error (TS1264, "Declarations
with definite assignment assertions must also have type annotations"). It is
nonetheless **valid by tsv's parse oracle** (acorn-typescript defers the
static-semantic early error, as does tsv — the permissive parser hands
correctness to tsc, not the formatter), and tsv keeps it stable:

```
b!;
```

Prettier's `typescript` parser (typescript-estree) **rejects** it at parse time:

```
Declarations with definite assignment assertions must also have type annotations.
```

so prettier cannot serve as a formatting oracle here — there is no
`output_prettier.*`. `prettier_rejects.txt` pins the error; rule F6 live-verifies
that prettier still rejects the input with that message, failing loudly if
prettier is ever relaxed or the error morphs.

This isolates TS1264 with no initializer to confound it: the initialized form
(`d! /* c */ = 1;`) additionally trips TS1263 and is
[property_definite_comment](../property_definite_comment_prettier_divergence/),
while the typed-and-initialized form (`e!: number /* c */ = 1;`) is the pure
TS1263 case,
[property_definite_typed_comment](../property_definite_typed_comment_prettier_divergence/).
The only valid definite form — annotation, no initializer (`c!: number;`) — both
formatters agree on and is covered by
[property_modifier_type_comment](../property_modifier_type_comment/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Prettier rejects valid input.
