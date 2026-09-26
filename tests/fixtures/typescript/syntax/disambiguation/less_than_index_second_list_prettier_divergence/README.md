# less_than_index_second_list_prettier_divergence

A second type argument list after a first list whose indexed access a `|` or `&` continues: tsv prints it as written, and prettier rewrites it to tsc's comparison.

tsv: `f<A[K] | B><C>(x)`, `new f<A[K] | B><C>(x)`
Prettier: `(f < A[K]) | (B > <C>x)`, `(new f() < A[K]) | (B > <C>x)` (script); `f<A[K] | B> < C > x` (template)

## Reason

**Semantic preservation.** The same divergence as [callee_type_args_second_list](../../../expressions/new/callee_type_args_second_list_prettier_divergence/), with a first list that holds an indexed access continued by a union or intersection. To acorn-typescript, whose tree Svelte compiles, `f<A[K] | B><C>(x)` calls the instantiation `f<A[K] | B>` with the type arguments `<C>`; tsc takes no type argument list ahead of a `<`, so to tsc the same text is the comparison `f < A[K] | B > <C>(x)`, with the angle-bracket type assertion `<C>(x)` on its right. tsv builds acorn's tree, and printed as written each parser reads the output as it read the input — the first list reads as a type argument list whatever its index spells, exactly as it does behind a string key (`f<A['k'] | B><C>(x)`) or with no index at all (`f<A | B><C>(x)`), the rows that pin the boundary.

Prettier's `<script>` parse is tsc's, so it prints tsc's comparison, which acorn-typescript reads as a comparison too — the component now compiles a comparison where it called `f`. In a template prettier prints `f<A[K] | B> < C > x`, an instantiation compared against `C` and `x`, another program again.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Instantiation expression parens).
