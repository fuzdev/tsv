# Type assertion vs. generic arrow (parenthesized-arrow operand) — Svelte Divergence

At a `<` in expression position acorn-typescript tries the generic-arrow
reading first, and its Babel-ported "abort on a parenthesized arrow" check is
dead code (acorn never sets `extra.parenthesized`), so `<any>(() => {})`
parses as an `ArrowFunctionExpression` whose `typeParameters` is `<any>`.
TypeScript (and Babel) instead read a type assertion in JSX-free `.ts`:
`<any>` is the asserted type, the parenthesized arrow the operand.

**tsv** keeps the TypeScript reading: `TSTypeAssertion` wrapping the arrow
(`expected_ours.json` vs `expected_svelte.json`). The contrast case
(`<any[]>(() => {})` — a type that cannot parse as type parameters) is an
assertion in both parsers.

The sibling forms acorn also reads as generic arrows — `<T>x => x` and
`<T,>(() => {})`, both TypeScript parse errors tsv rejects — are pinned in
`tests/type_assertion_arrow.rs` (a rejection can't be an `input_invalid_*`
fixture when the canonical parser accepts).

**Upstream**: @sveltejs/acorn-typescript — the `expr.extra?.parenthesized`
abort in `parseMaybeAssign`'s arrow `tryParse` never fires.

See [conformance_svelte.md](../../../../../docs/conformance_svelte.md) §TypeScript Corrections
(Type assertion vs. generic arrow).
