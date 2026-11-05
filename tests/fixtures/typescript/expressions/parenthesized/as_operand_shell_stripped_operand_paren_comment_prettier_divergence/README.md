# `as` / `satisfies` operand pair, comment in a stripped operand pair

The cast-operand twin of
[value_shell_stripped_operand_paren_comment](../value_shell_stripped_operand_paren_comment_prettier_divergence/):
a comment written last inside a redundant grouping pair down the right edge of an `as` or
`satisfies` operand stays inside the operand's pair after the operand, ahead of a comment
written after the pair, which follows on its own line (`(a + (b // c⏎) // d⏎) as T`). A
block written ahead of the comment after the pair takes the same place, and so does the
non-null operand's pair there (`(a + (b // c⏎) /* x */ // d⏎)!` prints
`a + b // c⏎\t/* x */ // d`), a member or a call after the `!` included (`(…)!.m`,
`(…)!()`). A block alone after the pair stays after the comment at `!`, in the order written,
where prettier's fixed point from the stripped-pair spelling puts it ahead of the `//`
(`(a + b) /* x */!; // c`).

tsv: `(⏎\ta + b // c⏎\t// d⏎) as T`, the two comments in the order written, each on its own
line — the form tsv gives the bare spelling (`(a + b // c⏎// d⏎) as T`)
Prettier: moves the two comments past the `;` and puts them on one line, where they weld into
one comment — from the input in the reverse order (`(a + b) as T; // d // c`, its first pass
parking them between the keyword and the type, `output_prettier` + `audit_signature`), from
the stripped-pair spelling in the order written (`audit_signature_shell`)

An own-line comment after the pair, and the non-null operand with nothing between its pair
and the comment, already keep the two apart, and with nothing written after the pair, at `as`
and at `!` alike, the comment trails the statement in both formatters — ahead of a member after
the `!` prettier keeps it after the `!` (`(a +⏎\tb)! // c⏎.m;`) — those are controls. A block
alone after the pair stays after the comment ahead of a member too.

See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Stripped-paren trailing line-comment convergence) and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
