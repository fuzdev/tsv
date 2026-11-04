# empty_slot_trailing_line_comment_after_semicolon_prettier_divergence

A `//` written on a `;`'s line in a C-style `for` header, before an **empty** test or
update slot, trails that `;` (`a; // c`), the way it trails a `;` that a present clause
follows. A later comment in the same slot keeps the slot's own line, and so does the slot's
comment when the `;` already carries one the clause before it hung there (`c9`, `c10`), or
when an own-line comment from that gap defers past the `;` (`c11`–`c16`);
`unformatted_ours_semicolon_line` writes those slot comments on the `;`'s line.

Prettier relocates by slot when the slot before that `;` is empty too, and its two answers
disagree: after an empty init, an empty test slot's comment floats down into the update
clause (`;⏎;⏎// c⏎i++`); after an empty test, an empty update slot's moves back up onto the
first `;` (`a; // c⏎;`). With a clause before the `;` both formatters trail it
([empty_slot_trailing_line_comment](../empty_slot_trailing_line_comment/)). A second comment in the slot is split
from the first and relocated too, or stranded between `)` and the body `{`.

tsv: keeps each `//` on the `;` it was written after
Prettier: moves it into the neighbouring clause, or onto the other `;`

## Reason

The `;`'s line is the authorship signal: the comment annotates that separator and the
slot it closes. It is the rule the fully-empty header already follows
([empty_clauses_comment](../empty_clauses_comment_prettier_divergence/)), at a header
with one or two clauses present; the run cells follow
[empty_slot_comment_run](../empty_slot_comment_run_prettier_divergence/).

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
