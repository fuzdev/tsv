# const type param arrow — trailing comma divergence

The same divergence as single_type_param_prettier_divergence, with a `const`
modifier: a single unconstrained const type param on an arrow function.

- tsv: `<const T>(x: T) => x`
- Prettier: `<const T,>(x: T) => x`

The `const` modifier is not a constraint, so prettier's `shouldForceTrailingComma`
still fires; tsv emits the bare form, which Svelte's parser accepts. Function
declarations (`literal`, `tuple`) take no trailing comma in either formatter —
only the arrow diverges. The pure-TS sibling const_type_param/ is bare in both
formatters (prettier knows the `.ts` filepath).

Reason: **Design choice**. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §TypeScript.
