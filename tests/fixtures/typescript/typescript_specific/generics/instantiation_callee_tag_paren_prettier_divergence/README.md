# instantiation_callee_tag_paren_prettier_divergence

An instantiation expression as a call's callee or a tagged template's tag: tsv keeps the pair around it, and prettier strips it.

tsv: `(f<T>)()`, `(a.b<T>)(c)`, `` (f<T>)`x` ``, `x = (f<T>)(c).d`
Prettier: `f<T>()`, `a.b<T>(c)`, `` f<T>`x` ``, `x = f<T>(c).d`

## Reason

**Semantic preservation.** Bare, the list is no longer an instantiation: `f<T>()` is a generic call and `` f<T>`x` `` a generic tagged template, each carrying `<T>` as its own type arguments, where the input calls (or tags with) the instantiated `f<T>`. tsc and acorn-typescript, whose tree Svelte compiles, both read the two spellings as two trees. Over an open optional chain the pair is also where the chain ends: `(a?.b<T>)(c).d` calls whatever the chain produced and throws when `a` is nullish, where prettier's `a?.b<T>(c).d` short-circuits the call and the member with it — a different program with the types stripped. That spelling, and the open chain as a tag (`` (a?.b<T>)`x` ``, whose stripped form does not parse), are pinned in [`tests/instantiation_callee_tag_parens.rs`](../../../../../instantiation_callee_tag_parens.rs), as tsv's wire for an instantiation over an open chain nests the chain differently from acorn-typescript's.

The pair strips where the bare spelling reads the same: an optional call takes an instantiation callee bare (`f<T>?.()` is the instantiation called optionally, to both parsers), and a member object keeps its pair in both formatters (`(f<T>).x`). A doubled pair collapses to one (`unformatted_ours_paren_shell`). A member chain over the call keeps the pair on its base when it breaks. An instantiation over a parenthesized BASE is the chain's own, so its list stays the call's (`(f)<T>()` → `f<T>()`, [instantiation_paren_base_call](../instantiation_paren_base_call/)), and a decorator's call is the [call_instantiation_callee_paren](../../decorators/call_instantiation_callee_paren_prettier_divergence/) fixture.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Instantiation expression parens).
