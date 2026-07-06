# Type assertion arrow operand (`<T>x => x`) — Svelte Divergence

At a `<` in expression position acorn-typescript tries the generic-arrow reading
first, and its Babel-ported "abort on a parenthesized arrow" check is dead code
(acorn never sets `extra.parenthesized`), so `<T>` followed by *any* arrow parses
as the arrow's type parameters. TypeScript (and Babel) instead read a type
assertion in JSX-free `.ts`.

**tsv** follows TypeScript: an arrow function is not a `UnaryExpression`, so it
cannot be the operand of a type assertion — `<T>x => x` (and `<T>async x => x`,
same error) is a parse error tsv rejects while acorn accepts it as a generic
arrow.

Because the canonical parser accepts the input, this rejection cannot be an
`input_invalid_*` fixture (which requires both parsers to reject); the
`tsv_rejects.txt` marker pins tsv's rejection while `expected_svelte.json` proves
acorn still accepts. The sibling
[type_params](../type_params_svelte_divergence/) fixture covers the `<T,>(…)`
form, and the parenthesized-arrow *accept* reading tsv keeps is
[type_assertion_paren_arrow](../../type_assertion_paren_arrow_svelte_divergence/).

**Upstream**: @sveltejs/acorn-typescript — the `expr.extra?.parenthesized` abort
in `parseMaybeAssign`'s arrow `tryParse` never fires.

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §TypeScript Corrections
(Type assertion vs. generic arrow).
