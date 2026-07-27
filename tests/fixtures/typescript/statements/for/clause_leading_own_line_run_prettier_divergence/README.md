# clause_leading_own_line_run_prettier_divergence

An **own-line** block comment leading a `for` clause keeps its line, and the header
expands around it. Prettier pulls the whole header — comments and clauses — onto one
line. A run the author glued onto that line stays glued in both.

tsv: keeps the authored line, header expands
Prettier: collapses the header onto one line

## Reason

An own-line comment is an authored break tsv preserves — the same rule that decides
every other own-line comment in the header (see the sibling
[clause_own_line_comment](../clause_own_line_comment/), where prettier expands too).
A block written *on* the `(`/`;` line takes the opposite outcome of the same rule and
keeps the header on one line, matching prettier — pinned by
[clause_leading_comment_run](../clause_leading_comment_run/).

Whether the members of a glued run stay on one line is not part of the divergence:
both formatters keep them together, since a comment the author glued to the previous
one is one remark, not two.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
