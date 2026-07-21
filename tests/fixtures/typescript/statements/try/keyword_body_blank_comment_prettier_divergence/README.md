# keyword_body_blank_comment_prettier_divergence

Blank lines around own-line comments in the `try` / `catch` / `finally` keyword→`{` gap.

tsv: keeps the comment in the gap where the author wrote it, applying the header→body blank rule
Prettier: absorbs the comment into the block body, collapsing the blanks with it

## Reason

The comment relocation itself is the [line_comment_absorbed](../line_comment_absorbed_prettier_divergence/)
divergence. This fixture pins the **blank-line** half of the same gap, which that fixture does not
cover — the two questions are independent.

tsv applies one rule here, the same one the `if`/`while` `)`→`{` gap applies
(`push_header_to_body_gap`):

- a blank **above** the first own-line comment is **dropped**, so a body block's `{` never sits
  below a blank (`unformatted_ours_blank_above.svelte` normalizes to `input.svelte`)
- a blank **between** two own-line comments is **preserved** — it separates two distinct remarks

Prettier is no oracle: it relocates the comments into the block body and collapses every blank in
the run, so it makes no statement about the authored gap at all.

Bare `catch` is used rather than `catch (e)` deliberately — a parameterized `catch` sends prettier
into the catch *parens* instead of the block body, which is a different absorption target and would
mix a second question into this fixture.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §"No blank above a body block's `{`".
