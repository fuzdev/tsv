# callback_signature_break_expands_call_prettier_divergence

A call whose last argument is an arrow whose body is a call, where the arrow's **signature**
will break — an authored-expanded default object, or an authored-expanded object type on a sole
parameter. Every argument breaks out onto its own line, in a plain call, a `new` expression and
a member chain, alone or after a preceding argument.

That is prettier's rule: the hug prints the signature with its line breaks removed, so a
signature that will break cannot hug (`ArgExpansionBailout` in `function-parameters.js`), and
the call falls through to `allArgsBrokenOut()`. tsv matches it for the default-object cells.

The divergence is the sole-parameter object-type cells, and it is the cataloged one: prettier
drops the authored break of an object type annotating a hugged sole parameter
(`fn((a: { b: C }) => g(a))`), and tsv keeps it. With the break kept, prettier's own rule
applies to it — the signature will break, so the arguments break out. A block body is the
control: tsv keeps it hugged (`fn((a: {⏎\tb: C;⏎}) => {`), the object still expanded.

- **tsv**: `fn(⏎\t(a: {⏎\t\tb: C;⏎\t}) => g(a)⏎);`
- **Prettier**: `fn((a: { b: C }) => g(a));`

`unformatted_ours_hugged` is the hugged authoring of every cell.

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md)
§Hugged sole-parameter object type.
