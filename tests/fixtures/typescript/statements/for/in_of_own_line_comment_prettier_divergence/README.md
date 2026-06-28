# in_of_own_line_comment_prettier_divergence

Line comments authored **inside** a broken `for-in`/`for-of` header — between the
binding and the `in`/`of` keyword, or between the keyword and the iterable
(`for (\n\tx // a\n\t\tin\n\t\tobj\n)`).

Prettier collapses the header back inline and relocates the interior line
comment out to after `)` (`for (x in obj) // a`); tsv keeps the header broken
with each comment where the author placed it (between the operands). A **block**
comment authored inline in the same gaps (`for (x /* a */ in obj)`) stays inline
in both formatters — no divergence.

## Reason

A line comment after `)` runs to end-of-line, so the body drops to the next line
in both. tsv treats the author's in-header comment placement as intentional,
consistent with its handling across if/else, try/catch, switch, for, while,
do-while.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
