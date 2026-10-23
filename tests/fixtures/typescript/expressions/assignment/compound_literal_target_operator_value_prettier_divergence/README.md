# compound_literal_target_operator_value_prettier_divergence

An object or array literal on a compound assignment's left, holding an assignment of its own: tsv keeps that assignment's operator, and prettier turns it into `=`.

tsv: `({ a: (b += c) } += x)`, `f(([(a += b)] += x))`
Prettier: `({ a: b = c } += x)` (two passes); from the bare `[a += b]`, `[a = b]`

## Reason

**Semantic preservation.** Only `=` reads its left as a destructuring pattern (ecma262 §13.15.1); a compound operator's left stays the literal it was written as, and acorn-typescript, whose tree Svelte compiles, keeps it one. An assignment inside that literal is an expression with its own operator. Prettier's TypeScript parser converts the left to a pattern anyway, and a pattern has no slot for an operator: the element or property value becomes a default, so `b += c` prints as `b = c`, a different program. From the parenthesized spelling it takes two passes (the first strips the pair, the second changes the operator); from the bare spelling (`unformatted_ours_bare_compound`) one. `variant_default` is prettier's output from the bare spelling — a different, valid program, which tsv leaves as written: a formatter does not put back an operator prettier deleted. An `=` element prints bare, as prettier prints the default it reads there (`[a = 1]`); prettier keeps a pair the author wrote around it (`[(a = 1)]`, `unformatted_ours_paren_default`) while it strips the same pair around a property value (`{ a: (b = c) }`), and tsv prints both bare — the same tree to acorn either way. A parenthesized compound element (`[(a += b)]`) keeps its pair in both formatters; on the bare spelling tsv puts back the pair an array literal's assignment element takes.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Compound assignment literal target).
