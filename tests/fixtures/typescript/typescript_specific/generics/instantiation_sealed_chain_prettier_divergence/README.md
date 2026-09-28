# instantiation_sealed_chain_prettier_divergence

An optional chain the author sealed with a pair, instantiated: tsv keeps the pair, and prettier strips it.

tsv: `(a?.b)<T>`, `(a?.b())<T>`, `(a?.b)<T>?.()`
Prettier: `a?.b<T>`, `a?.b()<T>`, `a?.b<T>?.()`

## Reason

**Semantic preservation.** The pair is where the chain ends. acorn-typescript, whose tree Svelte compiles, reads `(a?.b)<T>` as an instantiation of the chain's result and `a?.b<T>` as a chain that instantiates `a.b`, so the stripped spelling moves the chain boundary even where nothing follows it. At such a plain position that is the whole difference: acorn's tree alone, with the same runtime behavior and the same types to tsc's checker. (tsv's own parse reads both spellings as the sealed tree, a known parse divergence.) tsv keeps the pair there because it follows acorn's tree, the one Svelte compiles, as prettier itself keeps the non-null spelling `(a?.b)!<T>`. Where something follows, the bare spelling carries that into the chain too: a tag or a `new` callee written that way does not parse (`` a?.b<T>`t` ``, `new a?.b()<T>()`), so where the printer drops a redundant pair around the whole instantiation (`` new ((a?.b())<T>)`t`() ``, `new ((a?.b())!<T>)()`) the chain's own pair is what stays. The non-null spelling `(a?.b)!<T>` keeps its pair in both formatters; tsv keeps it wherever the chain is instantiated, the spine of a `new` callee or a tag included (`` new ((a?.b())!<T>)<T>`t`() ``, which prettier prints as `` new (a?.b())!<T><T>`t`() `` and then, on its second pass, as a comparison).

The spellings that normalize to these forms — the pair around the whole instantiation as well as the chain (`` new ((a?.b())<T>)`t`() `` → `` new (a?.b())<T>`t`() ``) — are pinned in [`tests/new_callee_instantiation_parens.rs`](../../../../../new_callee_instantiation_parens.rs): prettier's output from them does not parse, so no variant can carry them.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Instantiation expression parens).
