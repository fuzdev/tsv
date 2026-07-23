# as_satisfies_value_trailing_block_comment_prettier_divergence

A redundant paren shell around an `as`/`satisfies` cast type whose **trailing**
gap holds a block comment after the type, with **no leading line comment**
(`(A /* t */)`, and the leading-block form `(/* b */ A /* t */)` — plus the
double-nested variants). The sibling
[as_satisfies_value_trailing_comment](../as_satisfies_value_trailing_comment_prettier_divergence/)
covers the case that also carries a leading **line** comment; here there is none,
so bug188's line-comment hang seam never fires — a distinct mechanism.

**tsv**: strips the shell and, because a cast is a **value** position, defers the
trailing block past the statement `;` (via `line_suffix`), matching the
declarator's own value-to-`;` trailing-comment handling. A leading block trails
the keyword inline. The form is idempotent in one pass:

```ts
const a = x as A; /* t */

const c = x as /* b */ C; /* t */
```

**Prettier**: reaches the same fixed point but is **non-idempotent** on the paren
shell — its first pass lands the block before the `;`
(`x as A /* t */;`, the `prettier_intermediate_*` files) and its second pass moves
it past the `;` (`x as A; /* t */`, our `input.svelte`). tsv normalizes the shell
to the fixed point in a single pass, where prettier takes two.

The `unformatted_ours_*` variants are the paren shells (tsv normalizes them to
`input` in one pass; N6 confirms prettier's one pass does not); the
`prettier_intermediate_*` siblings pin prettier's two-pass path to `input`. The
other prefix / hang sites (`keyof`, mapped, `: T`, `is`, type-param, conditional
`extends`) keep a trailing block **inline** at a type position, so they are already
idempotent — only the two value-position casts need this.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
