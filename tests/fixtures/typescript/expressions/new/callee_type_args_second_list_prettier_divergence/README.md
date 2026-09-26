# callee_type_args_second_list_prettier_divergence

A second type argument list right after the first: tsv prints it as written, and prettier rewrites it to tsc's comparison.

tsv: `new f<T><U>(x)`, `f<T><U>(x)`
Prettier: `new f() < T > <U>x`, `f < T > <U>x` (script); `new f<T>()<U>(x)`, `f<T> < U > x` (template)

## Reason

**Semantic preservation.** The parsers read the text two ways. To acorn-typescript, whose tree Svelte compiles, `new f<T><U>(x)` constructs the instantiation `f<T>` with the type arguments `<U>`, `new (f<T>)<U>(x)`, and `f<T><U>(x)` calls `f<T>` with `<U>`; with the types stripped, the component runs `new f(x)` and `f(x)`. tsc takes no type argument list ahead of a `<` (`canFollowTypeArgumentsInExpression` refuses one unconditionally), so to tsc the same text is the comparison `(new f < T) > <U>(x)`, with the angle-bracket type assertion `<U>(x)` on its right. tsv builds acorn's tree, and printed as written each parser reads the output as it read the input (the exception is a chain a line break splits before a member or an operator, which the printer joins onto one line in acorn's reading — see the catalog entry). That includes a tag: `` new f<T><U>`x` `` gets no argument list, since to tsc an appended `()` would join the assertion's operand.

Prettier's `<script>` parse is tsc's, so it prints tsc's comparison, `new f() < T > <U>x`, on every row but one. That spelling is a comparison to acorn-typescript too, so the component now compiles a comparison where it constructed `f`. The exception is `new f<A<B>><U>(x)`, whose nested close tsc reads as `>>` (`(new f < A) < (B >> <U>(x))`): prettier's first pass prints `new f() < A < B >> (<U>x)`, which tsc and acorn-typescript both read as a CALL on the constructed object, `(new f())<A<B>>(<U>x)`, a third program, and its second pass, the fixed point, prints that call as `new f()<A<B>>(<U>x)` — two passes to converge, pinned by `audit_signature.txt`. In a template prettier prints `new f<T>()<U>(x)` (a call on the constructed object) and `f<T> < U > x`, other programs again.

`unformatted_ours_line_break` puts a line break between the two lists: neither parser reads it differently, and tsv joins the line. A member chain after the call keeps both lists (`f<T><U>(x).y`, comments in either list included).

A pair the author wrote around one of these spellings survives wherever the position would strip it (`(f<T><U>(x)) * 2`, `(new f<T><U>`x`).y`): to tsc the comparison reaches out of that extent. The one bare spelling tsv leaves as it is although tsc rejects it is a head ending an open optional chain (`a?.b<T><U>()`): a repair pair would cut the chain and move its short-circuit, so acorn's reading is kept and tsc goes on rejecting the output as it did the input. Both are pinned in [`tests/new_callee_type_args_follow.rs`](../../../../../new_callee_type_args_follow.rs).

Where tsc's parser rejects the bare spelling outright (`new a.b<T><U>()`, whose assertion `<U>` has no operand), acorn-typescript still accepts it, and tsv prints the pair both parsers read alike, `new (a.b<T>)<U>()`. That repair is pinned in [`tests/new_callee_type_args_follow.rs`](../../../../../new_callee_type_args_follow.rs). An authored pair is kept: [instantiation_paren_type_args_follow](../../../typescript_specific/generics/instantiation_paren_type_args_follow_prettier_divergence/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Instantiation expression parens).
