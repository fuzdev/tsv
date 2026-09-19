# Bare angle-bracket assertion as the left operand of `**` — Svelte Divergence

`<T>x ** 2` has two readings. tsc parses `(<T>x) ** 2` and REJECTS it — `A type assertion
expression is not allowed in the left-hand side of an exponentiation expression.`, the same
diagnostic family as the bare-unary `-x ** 2` — so prettier, a tsc front end, throws on it.
acorn-typescript accepts it as `<T>(x ** 2)`: its assertion operand is a unary-level parse
that consumes the `**`. `expected_svelte.json` records that tree.

tsv rejects, as it does the bare unary in the same position: the construct is an error in
every TypeScript file, local to the operand, and the two parsers that accept or diagnose it
do not even agree on what it is. Accepting would mean building one of two trees for code
no TypeScript program can hold. The parenthesized spellings — `(<T>x) ** 2` and
`<T>(x ** 2)` — parse alike everywhere, and the printer keeps the first one's pair
([exponentiation_type_assertion](../exponentiation_type_assertion_prettier_divergence/)).
`tsv_rejects.txt` pins tsv's own error.

See [conformance_svelte.md](../../../../../docs/conformance_svelte.md) §TypeScript
Corrections (a bare angle-bracket assertion as the left operand of `**`).
