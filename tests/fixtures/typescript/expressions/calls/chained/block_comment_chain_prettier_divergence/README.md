# block_comment_chain_prettier_divergence

Prettier requires 2 passes to reach stable output when stripping block comment
parens on member chains. tsv normalizes to the same stable output in a single pass.

Both formatters produce identical stable output. The divergence is only in normalization —
tsv reaches stability in one pass.

## Pattern

Not JSDoc-specific — any block comment triggers this behavior.

Source: `/* outer */ (/* inner */ (a).b).c(fn)`

After paren stripping, the inner comment is placed mid-chain:

- **Pass 1** (unstable): `a/* inner */ .b` (no space before `/*`, space before `.b`)
- **Pass 2** (stable): `a /* inner */.b` (space before `/*`, no space before `.b`)

Both passes break the chain at the same point; only comment-adjacent spacing differs.

## Reason

Stable quirk. Prettier's babel/typescript parser strips the grouping parens and repositions
the comment mid-chain. The first-pass output has different spacing than the final stable
form. tsv normalizes directly to the stable form from the original parens source.
