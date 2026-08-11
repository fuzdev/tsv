# Divergence: declaration head→body `{` own-line block, prettier keeps the break

An **own-line** single-line block comment in a function expression's or object method's
head→body-`{` gap (`let a = function ()⏎/* c */⏎{};`). A single-line block forces nothing
and the comment trails the head, so tsv collapses the authored breaks to the inline form
both formatters hold stable (`let a = function () /* c */ {};`).

Prettier keeps the author's break here, leaving the body `{` alone on its own line
(`let a = function () /* c */⏎{};` — `prettier_variant_own_line`, stable in one pass and
normalized back to input by tsv). It is the same comment position in both, so the
divergence is purely whether the unforced break survives — the
[§Authored breaks in value position](../../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position)
rule read at this gap.

These two constructs are the outliers of prettier's own answer: for a function
*declaration* and a class method — the same signature shape, the same emitter on tsv's side
— prettier relocates the comment out before the parameter list instead
([relocated](../declaration_head_body_own_line_block_relocated_prettier_divergence/)), and
for an `enum` / `namespace` it collapses the break on a second pass
([two_pass](../declaration_head_body_own_line_block_two_pass_prettier_divergence/)). tsv
gives all of them one answer.

A **multiline** block the author broke after is outside this rule and keeps its break —
which for these two constructs both formatters agree on
([declaration_head_body_multiline_block_break](../declaration_head_body_multiline_block_break/)).

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
