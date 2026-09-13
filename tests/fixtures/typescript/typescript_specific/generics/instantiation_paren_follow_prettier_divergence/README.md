# instantiation_paren_follow_prettier_divergence

Prettier strips the paren pair around an instantiation expression whatever token follows it; tsv keeps the pair when that token would re-lex the closing `>`.

tsv: `(fn<T>) + 1`, `(fn<T>) >= 1`, `(fn<T>)!`
Prettier: `fn<T> + 1`, `fn<T> >= 1`, `fn<T>!`

## Reason

**Semantic preservation.** A type argument list is only read as one when the token after its `>` cannot continue a comparison chain. Prettier's `fn<T> + 1` re-parses as `fn < T > +1` — a different program, and prettier's own second pass prints it that way. Its `fn<T> < 1`, `fn<T> > 1`, `fn<T> >= 1`, `fn<T> >> 1`, `fn<T> >>> 1` and `fn<T>!` do not parse at all (tsc: `Expression expected`). tsc's rule (`canFollowTypeArgumentsInExpression`): a `<`, `>`, `+` or `-` token never follows a type argument list, and a `!` starts an expression, so it does not either. acorn-typescript, tsv's parse oracle, additionally rejects `<<` there (`fn<T> << 1`, which tsc reads as an instantiation), so the pair stays ahead of `<<` too. Every other binary operator can follow one, so the pair strips there — the set `instantiation_operator_follow` pins from the bare side.

The axis is the join of two tokens, not the operand's node: a binary-left operand that merely ends on the close (`a * fn<T> + 1`, `-fn<T> + 1`) re-lexes the same way, so the pair wraps that whole operand, and a pair authored around the instantiation alone moves out to it (`unformatted_ours_inner_pair`).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Instantiation expression parens).
