# Spread grouped-operand comment divergence

When a parenthesized binary operand of a spread (`...`) carries a trailing
comment inside its parens (`[...(x + y /* c */)]`, `fn(...(x + y /* c */))`),
tsv keeps the comment where the author wrote it — inside the parens. The
parens are canonical here: both formatters wrap a binary spread argument in
parens (`...x + y` → `...(x + y)`); only the comment position differs.

Prettier 3.9 relocates the comment **outside** the parens, before the `]`/`)`
(`[...(x + y) /* c */]`, `fn(...(x + y) /* c */)`).

Reason: comment preservation. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation (Spread grouped operand) and §Comment Position Philosophy.
