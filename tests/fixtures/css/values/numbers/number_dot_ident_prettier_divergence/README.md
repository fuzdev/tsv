# number_dot_ident_prettier_divergence

The `<number>.<ident>` sequence (e.g. `1.px`, `1.foo`) is **invalid CSS**: per
CSS Syntax 3 §4.3.3 a `.` is only part of a number when followed by a digit, so
`1.px` tokenizes as three tokens — `<number 1>` `<delim .>` `<ident px>` — not a
dimension.

tsv: preserves the source (`1.px`, `1.foo`)
Prettier: merges into a dimension (`1px`, `1foo`)

Prettier models declaration numbers as numeric+unit nodes and strips the
trailing dot regardless of what follows. tsv only strips a trailing dot before a
number terminator (`1.` → `1`, `1.e1` → `1e1`); before an identifier it leaves
the source untouched. This also keeps `url(1.png)` intact — which Prettier does
too, since url is a special token.

## Reason

Spec violation. Both forms only arise in invalid CSS, and preserving the source
avoids inventing a dimension from a token sequence that isn't one. See
[conformance_prettier.md §CSS: Values](../../../../../../docs/conformance_prettier.md#css-values) ("Number dot-ident").
