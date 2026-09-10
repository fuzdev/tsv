# clause_declaration_tail_comment_run_prettier_divergence

A declaration as a brace-less clause body (`type` alias, `declare function`, the
shorthand `declare module 'm'` — all accepted by the canonical parser there), with a
line comment in the pre-`;` terminator gap. `input.svelte` is a fixed point for BOTH
formatters — each comment on its own line at the `if`'s level, in authored order — so
the divergence is entirely in how the pre-`;` authoring normalizes:

**tsv**: reaches the separated form in one pass — the clause tail's deferred run, the
same emission every other statement kind takes there
(`statements/if/clause_terminator_gap_own_line_run` is the match-fixture family).

**prettier**: its first pass **welds** — `// c1` and the `;`→`else` comment land on one
output line (`// c1 // c2`, the second comment becoming text inside the first), which
reparses as a single comment. That is content loss, so tsv does not follow it;
`unformatted_ours_pre_terminator.svelte` states the ours-only normalization and the
auto-generated chain marker pins prettier's own multi-pass landing from that authoring.

The later cases carry a run the author **glued** onto one line. It breaks between its
members for the same reason every clause tail does: the declaration's tail stays open to
the `else`, so the gap defers through `line_suffix`, and the same document read back with
the `;` ahead of the run is split by the ejected seam — one shape per document, whichever
way the terminator was written. See
[clause_terminator_gap_glued_comment_run](../clause_terminator_gap_glued_comment_run/)
for that rule's own fixture.

Reason: print-once over the weld, authored order preserved. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
