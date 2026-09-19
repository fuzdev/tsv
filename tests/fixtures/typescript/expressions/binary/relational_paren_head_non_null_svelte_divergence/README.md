# `(`-headed type-argument region whose content is a non-null `!` type — Svelte Divergence

`!T` and `T!` are tsc's `JSDocNonNullableType`. The compiler's PARSER accepts one
anywhere a type stands, so `a < (!b) > (t, u)` is a generic call over a parenthesized
type to tsc — prettier prints it as `a<!b>(t, u)` — and the rejection is its checker's
(TS8020, `JSDoc types can only be used inside documentation comments`), in every
TypeScript file. acorn-typescript has no such type: its type-argument parse fails, it
backs off, and it reads the comparison chain `a < !b > (t, u)`.

tsv follows **tsc** to the region and rejects there. The `(` head's body grade
refuses only what the compiler ABANDONS, and a `!` is a token its type grammar
carries; accepting the claim instead would need a type node no acorn-typescript wire
has, for a construct that is an error in every `.ts` file — an unconditional, local
error, which tsv's parser rejects rather than defers. `expected_svelte.json` records
what acorn-typescript keeps; `tsv_rejects.txt` pins tsv's own error.

The **shell-free** spelling is a different cell: `a < !b > (t, u)` opens the region
on the `!` itself, where tsv's parse follows acorn-typescript and reads the chain,
and the printer puts a pair around the `>`'s left operand so that no reader takes
the output for the call
([conformance_prettier_ts.md §Relational chain type-argument parens](../../../../../../docs/conformance_prettier_ts.md)).

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §TypeScript
Corrections (a type-argument region tsc claims and acorn-typescript abandons).
