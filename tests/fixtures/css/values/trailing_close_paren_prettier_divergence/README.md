# trailing_close_paren_prettier_divergence

A value ending in a `)` that closes nothing (`x )`, `screen )`).

The same `<)-token>` as [stray_close_paren](../stray_close_paren_prettier_divergence/),
at the end of the value: per CSS Syntax 3 §5.4.7 it is one more component value, and
tsv treats the whitespace before it as a separator (collapsed to one space) while the
rest of the value normalizes as usual (`(a: .50PX ) )` → `(a: 0.5px) )`) — in a
declaration value, with or without a comment, and in an `@import` prelude alike. Svelte's
`parseCss` accepts every line. Stable under tsv.

Where a stray `)` *followed by* another value makes prettier freeze the value verbatim,
one that ends the value makes prettier's postcss **throw**:

```
Unbalanced parenthesis
```

so it can't serve as a formatting oracle. `prettier_rejects.txt` pins the error; rule F6
live-verifies that prettier still rejects the input. The same throw on a `@supports`
prelude, which tsv keeps verbatim instead, is
[supports_unbalanced_paren](../../at_rules/supports_unbalanced_paren_prettier_divergence/).

See [conformance_prettier_ts.md §Prettier rejects valid input](../../../../../docs/conformance_prettier_ts.md#prettier-rejects-valid-input).
