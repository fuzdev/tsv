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

Design choice: strip parens that carry no meaning. Matches Biome; content is identical
(ASTs match) — only the redundant parens differ.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §TypeScript.
