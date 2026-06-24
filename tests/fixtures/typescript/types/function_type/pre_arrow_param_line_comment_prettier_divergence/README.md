# pre_arrow_param_line_comment_prettier_divergence

A line comment in the `)`→`=>` gap of a function-type or constructor-type that
**has parameters** (`(a: T) // c => void`) is preserved where the user placed it,
trailing `)`, with `=>` forced onto the next line. Prettier instead relocates the
comment **into** the parameter list, trailing the last parameter, and breaks the
params.

tsv: keeps the comment after `)` (`(a: T) // c\n=> void`)
Prettier: moves it onto the last param (`(\n\ta: T // c\n) => void`)

```
// tsv                  // prettier
type G = (a: T) // c    type G = (
=> void;                    a: T // c
                        ) => void;
```

## Reason

tsv treats user comment placement as intentional (see Comment Position
Philosophy): a comment parked after `)` is a trailing comment in that gap, and
moving it onto a parameter is a syntactic-position move. tsv preserves it in
place, which is idempotent.

The **empty-params** sibling matches Prettier (both keep `() // c\n=> void` — see
the regular fixture [pre_arrow_line_comment](../pre_arrow_line_comment/)); only
the with-params case diverges, because Prettier has a parameter to relocate the
comment onto. Prettier's relocation is also its own stable form here (it rewrites
the tsv shape to the relocated one in a single pass), so this is a clean
single-pass divergence, not an unstable intermediate.

Before this, the comment swallowed the following tokens (`(a: T) // c => void` —
the `=> void` absorbed into the comment, invalid and non-idempotent); now it is
preserved. Covers function types and constructor (`new (...)`) types.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
