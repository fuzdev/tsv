# private_fields_optional_chain_prettier_divergence

Optional chaining to a private field (`x?.#a`) is **valid modern JavaScript** —
the spec's `OptionalChain : ?. PrivateIdentifier` production (added with the
private-fields-in-`in` proposal). Our parser accepts it (matching Svelte /
acorn-typescript) and our formatter keeps it stable.

Prettier's `typescript` parser (typescript-estree) **rejects** it:

```
An optional chain cannot contain private identifiers.
```

so prettier cannot serve as a formatting oracle here — there is no
`output_prettier.*` to record. `prettier_rejects.txt` pins the error message;
rule F6 live-verifies that prettier still rejects the input with that message,
and fails loudly if prettier is ever fixed (accepts the input) or the error
morphs.

This is a **prettier-core / typescript-estree** bug, not a
prettier-plugin-svelte bug — it reproduces in plain prettier with
`parser: 'typescript'` (zero Svelte) and is fine under `babel-ts`. The 4.x
prettier-plugin-svelte bump surfaced it because the plugin switched
`lang="ts"` formatting from `babel-ts` to the real `typescript` parser.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Prettier rejects valid input.
