# operator_trailing_comment_blank_prettier_divergence

An author blank line between a binary operator's **trailing** line comment and the
own-line comment below it (`x + // c1⏎⏎// c2⏎y`). tsv keeps the blank; prettier
collapses it.

Prettier collapses only that **first** blank — the one directly under the
operator-trailing comment. A blank between two later comments in the same run
survives in both formatters (the `const c` case), as does one between two comments
that were both authored own-line (`const d`, a control where the formatters agree).

## Reason

Prettier prints the first comment through `printTrailingComment`, which has no
emitter for a blank line, and everything below it through `printLeadingComment`,
which does — so the collapse is a property of which printer the comment happens to
reach, not a judgement about the blank. This is the same shape as the `}`→`else`
gap, prettier's other blank collapse (§"A blank line BETWEEN two own-line comments
is always preserved").

tsv treats an authored blank between two comments as separating two distinct
remarks, exactly as a blank between two statements does, and preserves it in every
gap — including this one, where the run is emitted by the shared anchored-run
emitter (`Printer::push_anchored_trailing_run`, `RunLeadingBlank::Keep`). Prettier
itself preserves the blank wherever it *relocates* these comments, so its collapse
here is an inconsistency rather than a position rule tsv would have reason to copy.

The comment *positions* are unchanged in both formatters — only the blank differs.
The authored position itself is the subject of the sibling
[operand_operator_line_comment](../operand_operator_line_comment_prettier_divergence/),
whose `variant_comment_after_operator` pins that tsv keeps this operator-trailing
placement stable.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §"A blank line BETWEEN two own-line comments is always preserved".
