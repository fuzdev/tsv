# body_terminator_comment_run_prettier_divergence

A brace-less `do` body whose pre-`;` gap holds a same-line trailer and an own-line
line comment, with a third comment in the `;`→`while` gap — three comments reach one
flush at the break before `while`. The second case puts a **block** comment in the
`;`→`while` gap behind a deferred own-line tail: the anchor-line block rides the same
`line_suffix` machinery (an inline emission would render ahead of the buffer), so the
two flush in source order, the block on its own line at the `do`'s level.

**tsv**: collapses the body onto `do` (its standing do-while landing) and keeps each
comment on its own line, in authored order, at the `do`'s level:

```
do expr; // c1
// c2
// c3
while (a);
```

**prettier**: expands the body and relocates both own-line comments INTO the `while`
condition's parens (`while (⏎\t// c2⏎\t// c3⏎\ta⏎)`) — and from the pre-`;` authoring
its fixed point additionally REORDERS the pair (`// c3⏎// c2`, the two-pass chain
pinned in `audit_signature_pre_terminator.txt`). The `;` and the `while` head are
structure between the comments and the condition; tsv does not carry a comment across
them.

Printing the own-line pre-`;` comment as real text whose line closes only at the `while`
break welds the pair from the pre-`;` authoring (`// c2 // c3` — the second comment
becoming the first's text; `input` itself is stable either way), since the `;`→`while`
comment's deferred run flushes onto it. That authoring is the
`unformatted_ours_pre_terminator` form. In clause position the whole terminator-gap run defers through the same
`line_suffix` machinery, dedented to the construct's level, so the run meets the flush
in authored order with the separator breaking between members
(`doc/arena_render_suffix.rs`).

Reason: comment position and order preserved over prettier's relocation into the
condition, and print-once over the weld. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
