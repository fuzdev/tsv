# optional_paren_non_null_member_chain_prettier_divergence

A non-null assertion on a parenthesized optional-chain base, followed by a
**member chain that contains a call** (`(a?.b)!.c()`). Prettier drops the parens,
changing the optional chain's short-circuit boundary.

tsv: `(a?.b)!.c()` (preserves semantics — `.c()` calls on the asserted result of `a?.b`)
Prettier: `a?.b!.c()` (changes semantics — folds `.c()` into the chain)

The `!` only matters because it adds a node that pushes the chain past Prettier's
`member-chain.js` threshold; the member-only and single-call forms stay below it
and match in both formatters:

| Form            | tsv          | Prettier     |
| --------------- | ------------ | ------------ |
| `(a?.b)!.c()`   | keeps parens | drops parens |
| `(a?.b)!.c.d()` | keeps parens | drops parens |
| `(a?.b)!.c`     | keeps parens | keeps parens |
| `(a?.b)!.c.d`   | keeps parens | keeps parens |
| `(a?.b).c()`    | keeps parens | keeps parens |

## Reason

Semantic preservation. `(a?.b)!.c()` throws if `a` is null; `a?.b!.c()`
short-circuits the `.c()` tail to `undefined` — different ASTs (acorn seals the
chain inside the `TSNonNullExpression`). Same bug as the no-assertion sibling
[optional_paren_member_chain](../optional_paren_member_chain_prettier_divergence/):
Prettier's `member-chain.js` flattens the chain without honoring its own
`parentheses/chain-expression.js` rule (keep parens for a `ChainExpression`
that is the `object` of a non-optional `MemberExpression`). tsv keeps the parens.
See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§TypeScript (Optional-chain base member chain).

## Related

- `chain/optional_paren_member_chain_prettier_divergence/` — the no-assertion form `(a?.b).c.d()`.
- `chain/optional_paren_non_null_boundary/` — `(a?.b)!.c`, `(a?.b)!()`, `(a?.b)![c]`, `(a?.b)!.c.d`, where tsv and Prettier agree.
