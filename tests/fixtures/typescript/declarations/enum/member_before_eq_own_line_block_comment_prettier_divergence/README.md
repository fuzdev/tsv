# member_before_eq_own_line_block_comment_prettier_divergence

An own-line **block** comment in an enum member's name→`=` gap
(`A⏎/* c */⏎= 1`). A single-line block forces nothing, and a comment in this
gap trails the name (a trailing position), so tsv collapses the authored breaks
and keeps the comment inline in its authored syntactic slot (`A /* c */ = 1` —
the form both formatters hold stable when authored inline, pinned as a match in
[member_value_comment](../member_value_comment/)), reaching in **one pass** the
fixed point prettier itself converges to **non-idempotently** (pass 1 relocates
the comment across the `=`, gluing it to the operator with the value on the
next line — `A = /* c */⏎1,` — and later passes pull it back inline, one
comment per pass, so the single-comment member takes 2 passes and the run
takes 3).

The end states agree, so this is a pass-count divergence, not an end-state one:
`unformatted_ours_own_line.svelte` normalizes to input under tsv only (the
one-pass claim; prettier's first pass lands elsewhere), and no
`output_prettier.svelte` exists because input is prettier-stable. The run's
chain is too long for a `prettier_intermediate_*` pin (two distinct
intermediates), so the README records it — the same shape as
[key_colon_own_line_block_comment](../../../syntax/comments/key_colon_own_line_block_comment_prettier_divergence/).
Contrast the class-property and declarator siblings
([property_before_eq_own_line_block_comment](../../class/property_before_eq_own_line_block_comment_prettier_divergence/),
[declarator_before_eq_own_line_block_comment](../../variable/declarator_before_eq_own_line_block_comment_prettier_divergence/)),
where prettier instead relocates past the `=` to a stable hung-value form — the
enum member is the one before-`=` site where prettier comes back.

The same-gap **line** comment (which forces the break → continuation indent) is
[member_before_eq_line_comment](../member_before_eq_line_comment_prettier_divergence/).

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation.
