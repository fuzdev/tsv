# keyword_paren_comment_prettier_divergence

Prettier moves a comment between the `if` keyword and the condition's `(` to
inside the parens, before the condition (`if (/* c */ a)` instead of
`if /* c */ (a)`). tsv preserves comments where the user placed them.

tsv: `if /* c */ (a)` (comment kept before the paren)
Prettier: `if (/* c */ a)` (relocated inside the parens)

The second case (`/* (note) */`) additionally guards open-paren detection: a
`(` inside the comment must not be mistaken for the condition's opening paren.

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's
handling across if/else, try/catch, switch, for, while, do-while, labeled
statements, and call chains. `while` and `switch` behave identically.
