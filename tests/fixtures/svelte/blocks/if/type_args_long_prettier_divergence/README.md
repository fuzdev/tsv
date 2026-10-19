# type_args_long_prettier_divergence

A standalone `{#if …}` head over printWidth whose call's type arguments wrap. tsv wraps the
`<>` one argument per line and hugs the last argument after the `>`, then `)}`, as a single
call whose arguments wrap does. The pair at the boundary: with `>(` ending at column 100 the
type arguments stay inline and the argument breaks out; one column wider, the `<>` wraps and the
argument hugs. Prettier never width-wraps a block head — it keeps the whole
head inline past printWidth (and spaces a lone object argument's parens, `>( { a: 1 } )}`).

## Reason

See [conformance_prettier_svelte.md §Svelte: Blocks](../../../../../../docs/conformance_prettier_svelte.md#svelte-blocks) for the head-wrap model and why tsv diverges.

## Related

- [if/long](../long_prettier_divergence/) — the same divergence over the other head shapes
