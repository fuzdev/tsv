# optional_paren_non_null_bare_prettier_divergence

A non-null assertion `!` on a **bare** parenthesized optional chain — one with no
trailing non-optional access — has redundant parens. The `!` is TypeScript-only and
applies to the whole chain regardless of the parens (`(a?.b)!` ≡ `a?.b!`), so tsv drops
them. Prettier keeps them.

- **tsv**: `a?.b!`
- **Prettier**: `(a?.b)!` — kept (`prettier_variant_paren`)

`(a?.b!)` (the `!` authored inside the parens) strips in **both** formatters
(`unformatted_inside`).

This is the no-trailing-access case only. When a non-optional access follows
(`(a?.b)!.c`), the parens are **required** — they seal the chain so `.c` is not
short-circuited — and both formatters keep them (see
[optional_paren_non_null_boundary](../optional_paren_non_null_boundary/) and
[optional_paren_non_null_inside](../optional_paren_non_null_inside/)).

## Reason

Design choice: strip parens that carry no meaning. Matches Biome.

⚠️ The two spellings do **not** have identical ESTree ASTs — they nest the chain
boundary the other way round (`TSNonNull(ChainExpression(Member))` for `(a?.b)!`,
`ChainExpression(TSNonNull(Member))` for `a?.b!`), and that difference is exactly what
the required-paren case preserves the author's `!` placement for. The **bare** position
is the one place it is inert: with no access following, nothing can be short-circuited,
the `!` is erased at runtime, and the two type identically. The strip is sanctioned
there and nowhere else.

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript.
