# cast_prettier_divergence

`using` (and `await using`) followed by `as` or `satisfies` is a **cast
expression** of the identifier `using` (`using as T`, `(await using) as T`) —
valid by tsv's parse oracle (acorn-typescript takes the cast reading) and
stable under tsv.

Prettier's `typescript` parser (tsc) **rejects** it:

```
',' expected.
```

tsc commits to a *declaration* reading instead — a `using` declaration whose
binding is named `as`/`satisfies` (`using as = r;` parses in tsc) — and errors
when no `=` follows. acorn reads the cast and rejects the declaration form;
where the two oracles conflict, the drop-in oracle wins, so tsv reads the cast
too. Every other identifier-shaped word after `using` is a binding attempt in
both (the contextual-keyword binding cases live in
[basic_svelte_divergence](../basic_svelte_divergence/) and
[await_svelte_divergence](../await_svelte_divergence/)). `prettier_rejects.txt`
pins the error; rule F6 live-verifies that prettier still rejects the input.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Prettier rejects valid input.
