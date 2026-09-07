# escaped_name_terminator_prettier_divergence

A function whose **name** carries a hex escape and is glued to its `(` — `c\41(0.1)`,
`c\41x(0.1)`.

A hex escape takes up to six hex digits and then **optionally one whitespace character**,
which belongs to the escape rather than separating it from what follows (CSS Syntax 3
§4.3.7; "This optional whitespace allow hexadecimal escape sequences to be followed by
'real' hex digits"). `(` is not a hex digit, so the terminator is not needed here and
`c\41(` and `c\41 (` are the **same** `<function-token>` named `cA` — the first ends the
escape at its digits, the second inside the escape. tsv keeps whichever the author wrote:
glued stays glued here, and the spaced spelling keeps its space in the plain sibling
[escaped_name_terminator](../escaped_name_terminator/).

Prettier writes the terminator in regardless, printing the name and the parens as two nodes
separated by a space. On the first case that is lossless — the token is the same either way —
but on the second it is not: `c\41x` needs no terminator at all (the escape already ends at
the literal `x`), so its `(` is glued to the ident sequence and the run is the function token
`cAx(`. Prettier's `c\41x (0.1)` is an `<ident-token>` followed by a `<(-token>` — a
different token stream, and a value the UA no longer reads as a call. The insertion is the
same class of escape-blindness as
[escaped_delimiter_name](../escaped_delimiter_name_prettier_divergence/), where prettier
reads an escape's payload byte as structure; here it re-spells the escape's own boundary.

Measured, prettier inserts the space whenever the name carries a hex escape that is not
whitespace-terminated — `c\41`, `c\4`, `c\419`, `c\41abc`, `c\41_x`, `c-\41x` all take it —
and does **not** when the escape carries its terminator (`c\41 x`, the fixture-wide agreement
in [escaped_name](../escaped_name/)) or when a literal escape ends the name (`c\g`, `c\gx`).
The third case here is the observed exception: a name holding a `-` before its last character
(`c\41-x`, `c\41x-y`, `c\41--`) keeps its glue, while `c\41-` does not — a carve-out this
fixture pins as measured rather than explains.

See [conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values).
