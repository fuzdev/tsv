# index_signature_prettier_divergence

A decorator list in front of a **class index signature** (`@a [k: string]: T;`), once
inline and once on its own lines. acorn-typescript parses the member's decorator list
before it knows the member's kind and keeps it on the `TSIndexSignature` it builds, as a
`decorators` field after `typeAnnotation`. tsv keeps the list the same way and prints it
in the layout of any decorated member: inline when authored inline, one decorator per
line when authored on their own lines.

## Why tsv Differs

Prettier's `typescript` parser rejects the input outright — tsc parses the member and
reports TS1206 (`Decorators are not valid here`) from its grammar checker:

```
Decorators are not valid here.
```

tsv defers TS1206 to the diagnostics layer with its other placement rules, as it does
for a parameter decorator on a bodyless signature
([parameter_bodyless_signature](../parameter_bodyless_signature_prettier_divergence/)) and
a decorated static block ([static_block](../static_block_prettier_divergence/)). So there
is no `output_prettier.*`; `prettier_rejects.txt` pins the first error the file draws and
rule F6 live-verifies that prettier still rejects it.

**Svelte's parser** accepts the whole file, so `expected.json` is the ordinary canonical
AST and this is not a Svelte divergence.

See [conformance_prettier_ts.md §Prettier rejects valid input](../../../../../../docs/conformance_prettier_ts.md#prettier-rejects-valid-input),
and the frame's decision rules in
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md).
