# Type assertion arrow type params (`<T,>(() => {})`) — Svelte Divergence

At a `<` in expression position acorn-typescript tries the generic-arrow reading
first (its "abort on a parenthesized arrow" check is dead code), so `<T,>`
followed by a parenthesized arrow parses as the arrow's type parameters.
TypeScript instead reads this as a type assertion opening — but `T,` is not a
valid assertion type, and TypeScript does not backtrack into the generic-arrow
reading over a parenthesized arrow.

**tsv** follows TypeScript: `<T,>(() => {})` is a parse error (`Expected '>'`,
found `,`) tsv rejects, while acorn accepts it as a generic arrow. tsv does not
re-read the `<T,>` as type parameters over the parenthesized arrow.

Because the canonical parser accepts the input, this rejection cannot be an
`input_invalid_*` fixture (which requires both parsers to reject); the
`tsv_rejects.txt` marker pins tsv's rejection while `expected_svelte.json` proves
acorn still accepts. The sibling [operand](../operand_svelte_divergence/) fixture
covers the `<T>x => x` form, and the parenthesized-arrow *accept* reading tsv
keeps is
[type_assertion_paren_arrow](../../type_assertion_paren_arrow_svelte_divergence/).

**Upstream**: @sveltejs/acorn-typescript — the `expr.extra?.parenthesized` abort
in `parseMaybeAssign`'s arrow `tryParse` never fires.

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §TypeScript Corrections
(Type assertion vs. generic arrow).
