# callee_comparison_over_assertion_prettier_divergence

A second type argument list right after the first is no list: tsv reads tsc's comparison over a type assertion and pairs its `<` operand, and prettier prints the comparison bare.

tsv: `(new f() < T) > <U>x`, `(f < T) > <U>x`
Prettier: `new f() < T > <U>x`, `f < T > <U>x`

## Reason

**Semantic preservation.** tsc takes no type argument list ahead of a `<` (`canFollowTypeArgumentsInExpression` refuses one before it asks about a line break), so `new f<T><U>(x)` is the comparison `(new f < T) > <U>(x)`, with the angle-bracket type assertion `<U>(x)` on its right — on one line or with the `<U>` on the next — and `f<T><U>(x)` is `(f < T) > <U>(x)`. tsv follows tsc ([conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections), the entry "A `<`, `+` or `-` after a would-be type-argument close"); acorn-typescript, whose tree Svelte compiles, reads two lists on one `new`, `new (f<T>)<U>(x)`, and a call `(f<T>)<U>(x)`. The assertion takes everything a unary operand takes after its type: arguments (`<U>x(y)`), a sequence (`<U>(x, y)`), a template and a tag (`` <U>`x` ``, `` <U>x`t` ``), and the calls and accesses after them (`<U>x()()`, `<U>x.y`).

Printed bare, the comparison reads back as tsc's but not as acorn-typescript's, which takes the instantiation `f<T>` and compares it against `U` and `x`: another program. The pair around the `<` operand — the relational-chain rule — is the spelling every parser reads alike. A call or an access after the chain takes a pair around it by precedence (`((f < T) > <U>x)()`, `` ((new f() < T) > <U>`x`)?.()() ``, a block head `{#if ((f < T) > <U>x)()}`), and inside a `new` callee that pair encloses the chain (`` new ((f < T) > <U>x<T>)`t`() ``).

Prettier's `<script>` parse is tsc's, so it prints the same comparison bare, `new f() < T > <U>x` — which acorn-typescript, and so the compiled component, reads as the instantiation compared. It prints the template cells bare too (`{f < T > <U>x}`), and its next pass reads those with acorn-typescript, as the instantiation compared (`{f<T> < U > x}`) — two passes to converge, pinned by `audit_signature.txt`.

## Variants

`unformatted_ours_same_line` writes every cell bare (`new f<T><U>(x)`, `(f<T><U>(x))()`), and `unformatted_ours_line_break` also puts one `<U>` on its own line (`new f<T>⏎<U>(x)`) — the spellings whose reading tsc and acorn-typescript split on. tsv reads tsc's comparison from both and normalizes them to `input.svelte`. Prettier reads the template cells with acorn-typescript, so from both it lands on one stable form, `divergent_variant_acorn_call` (`{new f<T>()<U>(x)}`, `{f<T> < U > x}`), which tsv reads as tsc does and rewrites to a third form. tsv's tree for the bare spellings is pinned in `tests/instantiation_less_than_follow.rs`.

**Known gap**: a nested close, `new f<A<B>><U>(x)` (tsc's `(new f < A) < (B >> <U>(x))`), is no cell here. tsv reads it as tsc does but prints it bare, `new f() < A < B >> <U>x`, where the region `<A<B>>` closes on the shift, which acorn-typescript re-lexes as a type-argument list, so acorn-typescript does not read back the program tsv and tsc read. The pair that would settle it is the `<` twin of the relational-chain rule, unwritten (the TODO in [`needs_parens.rs`](../../../../../../crates/tsv_ts/src/printer/needs_parens.rs)).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens).
