# parenthesized_optional_chain_prettier_divergence

A decorator that is a **parenthesized optional chain continued by a call**
(`@((a?.b)())`) crashes prettier's estree printer:

```
Cannot read properties of undefined (reading 'type')
```

Our parser accepts it (matching Svelte / acorn-typescript) and our formatter
keeps it stable, so prettier cannot serve as a formatting oracle here — there
is no `output_prettier.*` to record. `prettier_rejects.txt` pins the error
message; rule F6 live-verifies that prettier still crashes with that message,
and fails loudly if prettier is ever fixed.

This is a **prettier-core** bug (the `estree` printer), not a
prettier-plugin-svelte bug — it reproduces in plain prettier with
`parser: 'typescript'` and zero Svelte (`class C { @((a?.b)()) x; }`) and is
fine under `babel-ts`. The trigger is narrow: a parenthesized optional chain
that is then *continued* (called or member-accessed) inside a decorator
(`@((a?.b)())`, `@((a?.b).c())`); `@(a?.b())`, `@((a?.b))`, and the
non-optional `@((a.b)())` are all fine. The 4.x prettier-plugin-svelte bump
surfaced it because the plugin switched `lang="ts"` formatting from `babel-ts`
to the real `typescript` parser.

The non-crashing parenthesized-decorator cases live in the sibling
`parenthesized` fixture.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Prettier rejects valid input.
