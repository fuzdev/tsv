# relational_chain_type_arg_parens_assertion_follow_prettier_divergence

A comparison chain whose right operand is an angle-bracket type assertion — `f < T > <U>z` — keeps a paren pair around its `<` operand; prettier prints it bare.

tsv: `a1 = (f < T) > <U>z`, `a3 = (new f() < T) > <U>z`
Prettier: `a1 = f < T > <U>z`, `a3 = new f() < T > <U>z`

## Reason

**Semantic preservation.** tsc takes a type argument list in an expression only where the next token can follow one (`canFollowTypeArgumentsInExpression`), and it refuses a `<` there before it asks about a line break: a list followed by `<` never makes sense to it. So to tsc, and to tsv, `f<T><U>(z)` is the comparison `(f < T) > <U>(z)` whose right operand is the type assertion `<U>(z)`, on one line or with the `<U>` on the next, after a name, a member, a `new` callee or inside an argument list. acorn-typescript — the parser Svelte uses — reads a second type argument list on any line, `(f<T>)<U>(z)`: a call, which a type strip turns into `f(z)`. That reading is its own miss, not a grammar tsv follows: see [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections) (the entry "A `<`, `+` or `-` after a would-be type-argument close").

Prettier's bare output keeps the ambiguity: tsc reads it as the input, but acorn-typescript reads `f<T>` as an instantiation compared against `U` and `z`, another program again. tsv's pair around the `<` operand is the spelling every parser reads as the same comparison over the assertion — the relational-chain rule.

## Variants

`unformatted_ours_same_line` writes every cell bare (`f<T><U>(z)`) and `unformatted_ours_line_break` puts the `<U>` on its own line (`f<T>⏎<U>(z)`) — the spellings whose reading tsc and acorn-typescript split on. tsv reads tsc's comparison from both and normalizes them to `input.svelte`; prettier lands on its bare chain. The Svelte-side reading of those variants has no fixture shape (a variant carries no parse divergence), so tsv's tree is pinned in `tests/instantiation_less_than_follow.rs`.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens).
