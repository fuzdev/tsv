# curried_callback_signature_long_prettier_divergence

A block-bodied arrow passed as the sole argument of a curried call
(`memoize(1000)((…) => {…})`) whose flat signature overflows the callee's line. tsv breaks
every argument out and keeps the signature on one line, at every position the call can sit.

Prettier exempts this shape (`isDecoratedFunction` in `function-parameters.js`): under a
single-declarator `const`, `export default` or `module.exports =` it keeps the arrow hugged to
the callee's `(` and breaks the parameter list instead. Under `let` or a member assignment it
breaks every argument out, as tsv does everywhere.

- **tsv**: `memoize(1000)(⏎\t(data: Data, id: string, skip: boolean): string[] => {⏎…⏎\t}⏎);`
- **Prettier** (`const`): `memoize(1000)((⏎\tdata: Data,⏎\tid: string,⏎\tskip: boolean⏎): string[] => {⏎…⏎});`

At 100 the hug holds in both formatters. `unformatted_ours_hugged` is the hugged authoring of
every 101 cell.

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md)
§Curried-callee callback signature.
