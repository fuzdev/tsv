# operator_trailing_block_comment_prettier_divergence

A block comment the author left **trailing a binary operator**, with a newline
after it (`alpha + /* c1 */⏎beta`) — the operator→operand gap, the mirror of
[operand_operator_line_comment](../operand_operator_line_comment_prettier_divergence/)'s
operand→operator one.

tsv: keeps the comment after the operator, where it was written, and reflows the
author's break — `alpha + /* c1 */ beta`
Prettier: relocates it **backward across the operator**, onto the left operand —
`alpha /* c1 */ + beta`

## Reason

Prettier's comment attachment makes a comment on the operator's line a *trailing*
comment of the left operand, so it prints before the operator; tsv treats the
authored position as signal and keeps it after the operator ([Comment Position
Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)).
That is the same stance, and the same gap pair, as the sibling above — there the
comment is authored *before* the operator and prettier moves it after; here it is
authored *after* and prettier moves it before. Prettier relocates in both
directions; tsv preserves in both.

Only the *position* diverges. The author's line break after the comment is reflowed
by both formatters (§Authored breaks in value position): the gap's run holds no
comment that owns a line — a block trailing the operator has content before it and
none after it but the operand — so nothing forces the chain open and width decides,
exactly as it does for a glued run
([operator_glued_comment_run](../operator_glued_comment_run/)). A comment the author
*did* give a line of its own still breaks the chain in both
([operand_own_line_block_comment](../operand_own_line_block_comment/)).

## Cases

An arithmetic operator (`+`) and a logical one (`&&`) — one binary-chain printer,
so the rule is uniform.

- `unformatted_ours_authored.svelte` — the authored form: tsv normalizes it to
  input, prettier to the variant.
- `variant_authored.svelte` — prettier's relocated form, dual-stable: a comment
  authored *before* the operator keeps that position in tsv too, so tsv holds
  prettier's output as-is and an already-prettier-formatted file does not churn.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
