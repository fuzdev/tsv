# call_instantiation_callee_paren_prettier_divergence

A decorator whose expression calls an instantiation, on a class and on a class member: tsv keeps the pair around the instantiation, and prettier strips it over two passes.

tsv: `@((f<T>)())`
Prettier: `@(f<T>())`, then `@f<T>()` on its second pass

## Reason

**Semantic preservation.** The decorator calls the instantiated `f<T>`; bare, the list is the call's own type arguments (`f<T>()` is a generic call), a different tree to tsc and acorn-typescript alike. Prettier strips the instantiation's pair on its first pass and the decorator's own on its second, so its output never reaches a form that reads as the input (`audit_signature.txt`). tsv's decorator keeps its pair around a call whose callee keeps one, so the spelling written without it (`@(f<T>)()`, `unformatted_ours_paren_call`) prints with it. It is the decorator position of [instantiation_callee_tag_paren](../../generics/instantiation_callee_tag_paren_prettier_divergence/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Instantiation expression parens).
