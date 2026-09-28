# call_callee_instantiation_parens_prettier_divergence

A `new` callee that is an instantiation expression over a call: tsv keeps the pair around it, and prettier strips it.

tsv: `new (f()<T>)()`, `new (a.b()<T>)()`, `` new (f()`x`<T>)() ``
Prettier: `new f()<T>()`, `new a.b()<T>()`, `` new f()`x`<T>() ``

## Reason

**Semantic preservation.** A `new` callee holding a call on its left spine keeps its pair, or the `new` takes that call's argument list as its own: `new (f())()` constructs what `f()` returns, where `new f()()` constructs `f` and calls the result. Both formatters keep that pair through a member (`new (a.b())()`, `new (f().C)()`), a non-null `!` and a tagged template (`` new (f()`x`)() ``) — the plain [call_callee_parens](../call_callee_parens/) and [call_callee_parens_ts](../call_callee_parens_ts/). An instantiation expression is one more link on that spine, and prettier stops walking at it: `new f()<T>()` is `(new f())<T>()` to tsc and acorn-typescript alike, a call — with the type arguments — on the constructed object, where the input constructs the instantiated result of `f()`. With the types stripped the component runs `new f()()` instead of `new (f())()`. tsv walks through the instantiation to the call and keeps the pair.

The cells are template expressions. In a `<script>` body acorn-typescript hoists the list off the parenthesized callee onto the `new`, a tree tsv's wire does not reproduce, so the same spellings there are asserted in [`tests/new_callee_instantiation_parens.rs`](../../../../../new_callee_instantiation_parens.rs). There is no `unformatted_*` variant: the bare spelling is a different program, not another spelling of this one.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Instantiation expression parens).
