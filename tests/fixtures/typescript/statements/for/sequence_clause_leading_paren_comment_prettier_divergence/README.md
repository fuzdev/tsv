# sequence_clause_leading_paren_comment_prettier_divergence

An own-line comment inside the grouping shell the parser **erases** ahead of a
`for` header sequence clause's **first** operand (`for ((⏎// c⏎a in b⏎), c; ;)`).

Both formatters agree on the fixed point: the shell is gone, so the comment leads
the clause on its own line and the operands stay on one line. They part on the way
there — **prettier's** first pass from the authored form drops every operand after
the first onto a continuation line (`(a in b),⏎\tc;`), a break its own second pass
takes back, so the authoring normalizes to `input.svelte` for tsv only
(`unformatted_ours_paren_shell`, with prettier's intermediate pinned beside it).

## Reason

The run the erased shell held is hoisted ahead of the clause and printed **outside**
the operand run's group, which is where the reparse reads it — the comment now sits
in the header's `(`→clause gap and leads the sequence node rather than its first
operand. A hardline emitted inside that group would break the operands apart for a
comment that, one pass later, no longer sits between them
(`docs/comments.md` §The left-spine shell run).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment Position Philosophy and
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
