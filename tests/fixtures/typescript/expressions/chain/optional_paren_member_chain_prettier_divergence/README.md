# optional_paren_member_chain_prettier_divergence

A parenthesized optional chain that is the base of a **member chain** (one with a
bare member access, like `(a?.b).c.d()` rather than `(a?.b).c()`). Prettier drops
the parens, changing the optional chain's short-circuit boundary.

tsv: `(a?.b).c.d()` (preserves semantics — `.c` reads the result of `a?.b`)
Prettier: `a?.b.c.d()` (changes semantics — folds `.c.d()` into the chain)

## Reason

Semantic preservation. `(a?.b).c.d()` throws if `a` is null; `a?.b.c.d()`
short-circuits the whole tail to `undefined` — different ASTs. It's a bug in
Prettier's `member-chain.js`, which flattens the chain without honoring its own
`parentheses/chain-expression.js` rule (keep parens for a `ChainExpression`
that is the `object` of a non-optional `MemberExpression`). The non-member-chain
forms keep their parens in both formatters — see
[optional_paren_boundary](../optional_paren_boundary/). See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §TypeScript
(Optional-chain base member chain).

## Related

- `chain/optional_paren_boundary/` — `(a?.b).c`, `(a?.b)()`, `(a?.b)[c]`, where tsv and Prettier agree.
