# callback_signature_break_hugged_body_expands_call_prettier_divergence

A call whose last argument is an arrow whose body hugs on its own — an object, an array or a
conditional body — where the arrow's sole parameter carries an authored-expanded object type.
tsv keeps the expansion, so the signature will break, and every argument breaks out onto its
own line, in a plain call, a `new` expression and a member chain, alone or after a preceding
argument. The object type annotating a sole destructured parameter does the same, under a plain
and a curried callee.

That is prettier's rule read on the break tsv keeps: the hug prints the signature with its
line breaks removed, so a signature that will break cannot hug (`ArgExpansionBailout` in
`function-parameters.js`), and the call falls through to `allArgsBrokenOut()`. Prettier itself
drops the authored break of an object type annotating a hugged sole parameter, so its
signature does not break and the call hugs with the object collapsed.

- **tsv**: `fn(⏎\t(a: {⏎\t\tb: C;⏎\t}) => ({ a })⏎);`
- **Prettier**: `fn((a: { b: C }) => ({ a }));`

`unformatted_ours_hugged` is the hugged authoring of every cell.

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md)
§Hugged sole-parameter object type.
