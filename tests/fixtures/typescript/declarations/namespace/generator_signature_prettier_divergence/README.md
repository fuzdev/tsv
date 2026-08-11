# generator_signature_prettier_divergence

A **bodiless** generator signature — `function* gen(): Iterator<number>;` — nested in
a `declare namespace`, a `declare module`, a `declare global`, and a plain
`namespace`. The function carries no `declare` keyword of its own, so it is an
ordinary overload-shaped signature that happens to sit in a namespace body.

Both tsc diagnostics that reach these shapes are **checker** grammar errors, not
parse errors — TS1221 ("Generators are not allowed in an ambient context") in the
three ambient bodies, TS1222 ("An overload signature cannot be declared as a
generator") in the plain one — and `ts.createSourceFile(…).parseDiagnostics` is empty
on all four. tsv defers both, per its permissive-parser stance, and parses each as a
`TSDeclareFunction` with `generator: true`. **Acorn-typescript** accepts too, so this
is not a Svelte divergence.

Prettier's `typescript` parser (typescript-estree) **rejects** all four at parse time:

```
A function signature cannot be declared as a generator.
```

so there is no `output_prettier.*`. `prettier_rejects.txt` pins the error; rule F6
live-verifies that prettier still rejects with that message.

Note the message differs from the one the **top-level** `declare function*` form
draws ("Generators are not allowed in an ambient context") — typescript-estree
promotes a different grammar check there. That form is
[declare/function/generator](../../../typescript_specific/declare/function/generator_prettier_divergence/),
and the one member of the family prettier *accepts* — a `declare class` generator
method — is the ordinary
[declare/class/generator_members](../../../typescript_specific/declare/class/generator_members/).

The body-bearing counterpart in the same position is a different divergence with a
different oracle split (acorn rejects it):
[function_body](../function_body_svelte_divergence/).

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md)
§Prettier rejects valid input.
