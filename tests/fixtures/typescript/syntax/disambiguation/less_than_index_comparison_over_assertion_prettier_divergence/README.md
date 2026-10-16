# less_than_index_comparison_over_assertion_prettier_divergence

A second type argument list after a first list whose indexed access a `|` or `&` continues: tsv reads tsc's comparison in a template as in a `<script>`, and prettier's template keeps acorn-typescript's reading.

tsv: `(f < A[K]) | (B > <C>x)` (script and template)
Prettier: `(f < A[K]) | (B > <C>x)` (script); `f<A[K] | B> < C > x` (template, from the bare `f<A[K] | B><C>(x)`)

## Reason

**Semantic preservation.** The same reading as [callee_comparison_over_assertion](../../../expressions/new/callee_comparison_over_assertion_prettier_divergence/), with a first list that holds an indexed access continued by a union or intersection. tsc takes no type argument list ahead of a `<`, so `f<A[K] | B><C>(x)` is the comparison `(f < A[K]) | (B > <C>(x))`, the type assertion `<C>(x)` on the right of its `>`; tsv follows tsc ([conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections), the entry "A `<`, `+` or `-` after a would-be type-argument close"). acorn-typescript, whose tree Svelte compiles, reads a call of the instantiation `f<A[K] | B>` with the type arguments `<C>`. The first list reads the same whatever its index spells — behind a string key (`f<A['k'] | B><C>(x)`) or with no index at all (`f<A | B><C>(x)`), the rows that pin the boundary.

Printed with its pairs, the comparison reads alike to every parser, and prettier's `<script>` parse, which is tsc's, prints the same pairs, so `input.svelte` is prettier's too. The template is where the two part: prettier reads a template expression with acorn-typescript, so from the bare spelling (`unformatted_ours_same_line`) it prints `f<A[K] | B> < C > x` — an instantiation compared against `C` and `x`, a third program — which it keeps (`prettier_variant_same_line`), and from which tsv lands back on `input.svelte`.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens).
