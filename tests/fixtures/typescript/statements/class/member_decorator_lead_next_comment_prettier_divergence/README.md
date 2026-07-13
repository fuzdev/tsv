# member_decorator_lead_next_comment_prettier_divergence

A block comment on the same line as the *following* class-member decorator
(`@fn1⏎/* c */ @fn2`, or the all-inline `@fn1 /* c */ @fn2` that normalizes to
it).

- Input: `@fn1⏎/* c */ @fn2⏎a = 1;` (the comment leads `@fn2` inline)
- Prettier: `@fn1⏎/* c */⏎@fn2⏎a = 1;` (pushes the comment to its own line)
- Ours: `@fn1⏎/* c */ @fn2⏎a = 1;` (keeps it inline-leading `@fn2`)

## Reason

tsv attaches a between-decorator comment to the decorator it shares a line with,
consistently for **both** class-level and class-member decorators — a block
comment on the next decorator's line leads it inline. Prettier keeps this inline
form for class-*level* decorators (`@a⏎/* c */ @b`) but pushes it to its own line
for class *members*, and is **non-idempotent** on the all-inline authoring
(`@fn1 /* c */ @fn2`): pass 1 leads the next decorator inline, pass 2 moves it
own-line. tsv's inline-leading form is stable (idempotent) and consistent across
decorator contexts, so it is the more defensible choice.

tsv **preserves both authorings** (dual-stable): the own-line form
(`@fn1⏎/* c */⏎@fn2`, `variant_own_line`, identical to prettier's `output_prettier`)
is kept own-line, and the inline form (the input) is kept inline. Prettier collapses
the inline authoring onto the own-line form but keeps the own-line one — so tsv
respects the author's inline placement where prettier discards it.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
