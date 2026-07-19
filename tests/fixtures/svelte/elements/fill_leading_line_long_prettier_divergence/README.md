# fill_leading_line_long_prettier_divergence

Svelte text on both sides of an `{expression}` tag, so the fill is built with a **leading
separator** (`leading_line`). That shifts the fill's content/separator parity by one: every
`line` occupies a content slot and every word a separator.

The first `<li>` fits, so the leading `line` before `ccccc` renders flat, as a space. The
second overflows, and the `line` must become the **newline itself** — not a newline followed
by the space it stood for, which would strand a leading space at the head of the continuation
line (`~{expr}⏎\t ccccc`). A stranded space is read as indentation on the next pass and
dropped, so the format would have no fixed point.

At the overflow, tsv breaks before `~{yyyyyyyyyy}` to hold every line ≤ 100. Prettier keeps the
tag on the line and overflows to 101:

```
tsv       `…bbbbb bbbbb` / `~{yyyyyyyyyy}` / `ccccc ccccc ccccc ccccc`   all ≤ 100
Prettier  `…bbbbb bbbbb ~{yyyyyyyyyy}` / `ccccc ccccc ccccc ccccc`       101 chars
```

## Reason

Print width is a hard limit in tsv. See
[conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy),
which lists "fill algorithm edge cases (101-char boundaries)" among the constructs it governs.
The parity-shifted tail case — text after a tag as an element's *last* child — is pinned
separately by
[fill_tail_after_expr_long](../fill_tail_after_expr_long_prettier_divergence/).
