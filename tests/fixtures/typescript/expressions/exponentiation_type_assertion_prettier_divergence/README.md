# exponentiation_type_assertion_prettier_divergence

An angle-bracket type assertion as the LEFT operand of `**` keeps its paren pair:

```ts
// tsv                      // prettier
let a = (<T>x) ** 2;        let a = <T>x ** 2;
let e = (<T>(<U>x)) ** 2;   let e = <T>(<U>x) ** 2;
let f = 2 ** ((<T>x) ** 2); let f = 2 ** (<T>x ** 2);
```

## Why tsv differs

◆prettier_bug. The bare spelling is no longer the program the author wrote, for any parser
that reads it:

- **tsc** rejects it — `A type assertion expression is not allowed in the left-hand side of
  an exponentiation expression.`, the TypeScript twin of the bare-unary rule (`-x ** 2`) —
  so prettier's own second pass throws on its output (F4b tolerates the missing
  `audit_signature.txt`);
- **acorn-typescript** — tsv's parse oracle and Svelte's parser — accepts it as a DIFFERENT
  tree: its assertion operand is a unary-level parse that consumes the `**`, so `<T>x ** 2`
  is `<T>(x ** 2)`, the assertion over the whole exponentiation.

tsv rejects the bare form itself (see
[exponentiation_type_assertion_bare](../exponentiation_type_assertion_bare_svelte_divergence/)),
so the pair is what keeps its own output parseable too. As the RIGHT operand
(`2 ** <T>x`) or ahead of a looser operator (`<T>x * 2`) the assertion is an ordinary operand
and stays bare in both tools — carried here as controls, unchanged in
`output_prettier.svelte`.

## Reason

◆prettier_bug. See
[conformance_prettier_ts.md §TypeScript](../../../../../docs/conformance_prettier_ts.md#typescript)
(Exponentiation type-assertion operand) and
[conformance_prettier.md §Prettier bug index](../../../../../docs/conformance_prettier.md#prettier-bug-index).
