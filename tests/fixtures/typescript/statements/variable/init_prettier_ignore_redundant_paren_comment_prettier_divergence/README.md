# init_prettier_ignore_redundant_paren_comment_prettier_divergence

The sibling of
[init_prettier_ignore_paren_comment](../init_prettier_ignore_paren_comment_prettier_divergence/)
at the gap where the pair does **not** survive: an own-line `prettier-ignore` freezes the
initializer, the author wrapped it in a paren the value does not need, and a comment sits
between the frozen slice and that `)`. The shell is redundant, so it strips — and the
comment has no surviving `)` to stay inside.

Both tools answer the **block** spelling the same way: the comment defers past the
statement's `;` (`bbb  +  ccc; /* c */`). They part on the **line** spelling.

## Why tsv differs

A `//` deferred past the `;` lands on a line it does not own — anything already trailing
there merges with it, and the second comment stops being a comment. So tsv **retains** the
shell on the comment's own account and keeps the `//` inside it, the closer on its own
line. Prettier strips the shell anyway and carries the comment out past the `;`
(`eee  +  fff; // c`).

That is the same parting its **unfrozen twins** already record —
[init_assignment_paren_line_comment](../init_assignment_paren_line_comment_prettier_divergence/)
for the line spelling and
[value_paren_trailing_block_comment](../../../syntax/comments/value_paren_trailing_block_comment_prettier_divergence/)
for the block's landing past the `;` — and the freeze does not change it: whether the shell
is retained is a question about the gap's content, not about what renders between the
parens.

`unformatted_ours_paren` is the authoring both cases start from — the block one still
parenthesized. Prettier does not converge on it in one pass (it emits
`bbb  +  ccc /* c */;` and only moves the comment past the `;` on the next), so that chain
is pinned by `audit_signature_paren.txt` rather than by a single-form marker.

## Reason

◆comment_preservation — tsv preserves the authored position wherever relocating it would
merge two comments into one. Sanctioned in
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *On assignment-family value heads*); the governing principle is
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy),
and the unfrozen rule this one inherits is cataloged at
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
