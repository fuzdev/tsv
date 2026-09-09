# parameter_decorator_own_line_run_prettier_divergence

An own-line comment **run** in a parameter's decorator gap, authored with the decorators and the
binding otherwise flat (`@fn1 /* t */⏎/* c1 */ /* c2 */⏎@fn2 p`). tsv gives the run its own line;
prettier does too when the run is a single comment, and **collapses** it onto the decorator line
when the author glued two comments together.

- **tsv**: `@fn1 /* t */⏎/* c1 */ /* c2 */⏎@fn2⏎p` — the run keeps the line it was written on
- **Prettier**: `pair(@fn1 /* t */ /* c1 */ /* c2 */ @fn2 p) {}` — run, decorators and binding on one line

`input` is the broken-out form and is a **plain match**: there `@fn2⏎p` puts a newline directly
after a decorator, so prettier's `hasNewlineBetweenOrAfterDecorators` fires and forces the group
open. The divergence lives only in the flat authoring, `unformatted_ours_compact`, which tsv
normalizes to `input` and prettier normalizes to the dual-stable
`variant_prettier_pair_collapse`.

Two controls in the same document carry the argument: the class **member** and class-**level**
twins of the very same authoring, where prettier keeps the glued run on its own line.

## Reason

Prettier's answer turns on which of its comment-attachment handlers claims the run, not on
anything an author can see. At a class member and at the class level the run attaches as the
preceding decorator's **trailing** comments, and `printTrailingComment`'s own-line branch emits a
`hardline` — so the run always keeps its line. A plain parameter has no handler, so the run
attaches as the next decorator's **leading** comments, and `printLeadingComment` picks the
separator per comment: `hardline` when the comment has a newline both before and after it, a soft
`line` when it has one only after. The run's *last* comment is what the group sees, and a glued
pair (`/* c1 */ /* c2 */`) puts `*/` rather than a newline behind `c2` — so the separator softens
and the whole run collapses if it fits. One comment keeps its line; two glued comments do not.

tsv asks one question at all three decorator printers — is there an own-line run in this gap? —
and answers it the same way everywhere, so the parameter path agrees with the member path for
identical authoring. Collapsing the run is also a position loss: `c1` and `c2` end up
indistinguishable from a trailing run on `@fn1`, and the author's own-line placement is gone.

This is the same shape as the decorator-gap author blank in
[decorator_comment_blank](../../../statements/class/decorator_comment_blank_prettier_divergence/) —
one authoring, a different prettier verdict per attachment handler, one uniform tsv rule.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation.
