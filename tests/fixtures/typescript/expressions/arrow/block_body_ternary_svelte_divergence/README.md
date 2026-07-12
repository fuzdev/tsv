# Ternary after a bare arrow (`() => {} ? b : c`) — Svelte Divergence

An `ArrowFunction` is a complete `AssignmentExpression` (ecma262 §13.15 — a
top-level `AssignmentExpression` alternative, not a `ConditionalExpression`,
binary operand, or `LeftHandSideExpression`). So a bare, unparenthesized arrow
cannot be the test of a ternary: `() => {} ? b : c` is a syntax error. tsc and
prettier reject it (`Expected ';'` / TS1005), and **tsv rejects it** to match the
compiler.

acorn-typescript, however, *accepts* it: its arrow guard lives only in
`parseExprOps` (which stops a binary operator from applying to a leading arrow),
while `parseMaybeConditional` sits *above* that guard and still folds a trailing
`?` onto the arrow as a `ConditionalExpression` test. This is acorn over-leniency
in the accept direction — the reverse of most parser corrections, where tsv is
broader.

Because the canonical parser accepts this input, the rejection cannot be an
`input_invalid_*` fixture (which requires *both* parsers to reject). This
`tsv_rejects.txt` fixture pins the divergence from the other side: tsv rejects
(`tsv_rejects.txt` substring), while `expected_svelte.json` proves acorn still
accepts (a `ConditionalExpression` whose test is the arrow). The operator /
assertion / assignment-target forms (`() => {} || a`, `() => {} as T`,
`() => {} = a`), where acorn agrees and rejects, are the ordinary drop-in
rejections pinned by the `input_invalid_*` cases in the sibling
[block_body_not_operand](../block_body_not_operand/); subscripts and calls on a
bare arrow are pinned by
[block_body_not_callable](../block_body_not_callable/).

**Upstream candidate**: @sveltejs/acorn-typescript — `parseMaybeConditional`
folds a ternary onto an unparenthesized arrow above the `parseExprOps` arrow
guard.

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §TypeScript Corrections
(Arrow function as an operand).
