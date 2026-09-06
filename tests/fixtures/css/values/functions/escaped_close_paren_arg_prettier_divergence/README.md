# escaped_close_paren_arg_prettier_divergence

A function whose argument list contains an **escaped** `)` (`fn(a\)b, 0.1)`).

Per CSS Syntax 3 the `\)` is a valid escape (`\` + a non-newline) whose escaped code point
is the paren itself, and §"Consume an ident sequence" takes it as ident content — so `a\)b`
is the single ident `a)b` and the function closes at the final *unescaped* `)`: the value is
`fn(a\)b, 0.1)`, a two-argument function. Svelte's `parseCss` accepts
it (matching the spec) and tsv keeps it stable, normalizing the arguments the way it
does for any other function.

Prettier's CSS parser (postcss, not `typescript`) miscounts the escaped `)` as a
closing paren and throws:

```
Unbalanced parenthesis
```

Same bug and same fixture shape as
[url_escaped_paren](../url_escaped_paren_prettier_divergence/), one construct over — an
ordinary function's argument list rather than an unquoted `url()`. The sibling
[escaped_paren_arg](../escaped_paren_arg_prettier_divergence/) is the escaped **open**
paren, where prettier does not throw but stops normalizing.
`expected.json` is the parseCss AST; `prettier_rejects.txt` holds prettier's error
substring (no `output_prettier.*` — prettier can't format it).

See [conformance_prettier_ts.md §Prettier rejects valid input](../../../../../../docs/conformance_prettier_ts.md#prettier-rejects-valid-input).
