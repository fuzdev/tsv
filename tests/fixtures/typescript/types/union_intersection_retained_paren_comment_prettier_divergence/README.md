# union_intersection_retained_paren_comment_prettier_divergence

Block comment inside a **retained** parenthesized union member — a `(x | y)`
whose parens are kept because it nests inside an outer union or intersection
(`a | (b | c /* c */)`, `(a | b /* c */) | c`, `a & (b | c /* c */)`).

**Prettier** hoists a **trailing** comment out of the parens to after `)`
(trailing the whole member); a **leading** comment stays inside the parens
(`a | (/* c */ b | c)` — unchanged, matching tsv). **tsv** keeps the comment
where the user wrote it, inside the parens.

Per Comment Position Philosophy: the comment is inside the parenthesized member,
so tsv associates it with that member rather than hoisting it to the surrounding
union/intersection. Contrast `union_intersection_parens_comment`, where the parens
are redundant and stripped, so both formatters agree.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
