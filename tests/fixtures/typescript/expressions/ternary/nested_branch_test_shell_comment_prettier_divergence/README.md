# Nested branch test shell, block comment

A block comment inside the redundant paren shell around a **nested** conditional's test
(`a ? b : (/* c */ aaa) ? bbb : ccc`). Both formatters strip the shell, which leaves the
comment in the enclosing `?` / `:` branch gap, and both settle on the same form — the
comment laid out by that gap's rules, a glued multi-line block leading the branch at its
indent and an own-line single-line block collapsing with it. The difference is the **pass
count**: prettier's first pass still prints the comment inside the nested test's
`align(2)` (the multi-line block's closing line two columns in, the own-line block
breaking the branch open), and only its second pass, reading the shell-free output, lands
it in the gap. tsv reaches the fixed point in one pass, so `unformatted_ours_test_shell`
is a pass-count gap, pinned by `prettier_intermediate_test_shell`, not a relocation.

`y4` is the shell's other gap: a block AFTER the nested conditional, before the shell's `)`,
at a statement tail. Both formatters strip the shell and float the block past the `;`
(`ccc; /* t */`), the answer every stripped value shell gives there; tsv in one pass,
prettier in two (its first keeps `ccc /* t */;`).

An own-line `//` inside the same shell is the cataloged own-line branch rule instead —
[branch_own_line_line_comment](../branch_own_line_line_comment_prettier_divergence/).

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
("Nested conditional test shell, block comment").
