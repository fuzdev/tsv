# escaped_whitespace_prettier_divergence

A CSS value containing an **escaped whitespace** — `\` followed by a space.

Per CSS Syntax 3 [§4.3.4 "Check if two code points are a valid escape"](https://www.w3.org/TR/css-syntax-3/#starts-with-a-valid-escape)
(only a newline after `\` invalidates it) and [§4.3.7 "Consume an escaped code
point"](https://www.w3.org/TR/css-syntax-3/#consume-escaped-code-point) (whose
final branch returns the code point itself), `\ ` is a valid escape whose escaped
code point **is that space**. So the trailing space in `width: 50px\ ;` is value
*content*, not padding — the value is the ident `50px ` and the `;` terminates the
declaration.

tsv preserves it, so `input.svelte` formats to itself.

**Prettier drops the escape's payload**, which strands the backslash onto whatever
follows:

| source | prettier | consequence |
| --- | --- | --- |
| `width: 50px\ ;` | `width: 50px\;` | `\;` escapes the terminator — the declaration never ends |
| `height: var(--a, 50px\ );` | `height: var(--a, 50px\);` | `\)` escapes the closer — the function never closes |
| `margin: calc(1px\ );` | `margin: calc(1px\);` | same |
| `color: a\ b;` | `color: a\b;` | not a delimiter, but the value silently changes from `a b` to `ab` |

The first three make prettier's own output **fail to re-parse**: tsv's CSS parser
rejects `output_prettier.svelte` with `Expected '}'`, because with `;` and `)`
escaped the declaration and the block never close. (Svelte's error-recovering
`parseCss` still consumes it, so the AST oracle is unaffected — this is a
*formatter* divergence, not a parser one. tsv's parse AST matches `parseCss` on
`input.svelte` exactly, escaped space and all.)

tsv declines to reproduce it: **its format→re-parse invariant outranks matching
prettier.** Emitting output that does not parse is never the defensible side,
whatever the reference formatter does — and the repo's CSS stance is
spec-over-prettier where the two disagree.

`output_prettier.svelte` pins prettier's corrupt output; `input.svelte` is tsv's
stable form.

See [conformance_prettier.md §CSS: Values](../../../../../docs/conformance_prettier.md#css-values).
