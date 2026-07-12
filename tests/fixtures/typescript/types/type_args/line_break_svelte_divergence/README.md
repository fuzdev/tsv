# Type-reference type arguments after a line break (`B` ⏎ `<T>`) — Svelte Divergence

A type-argument list `<…>` binds to the preceding type reference only when no
line terminator intervenes. TypeScript's `parseTypeArgumentsOfTypeReference` is
guarded by `!scanner.hasPrecedingLineBreak()`, so `B` followed by `<T>` across a
newline is **not** `B<T>` — the type is just `B`, and the `<T>` begins a new
construct.

**tsv** follows tsc and prettier: after `let a: B`, a line break ends the type at
`B`, leaving `<T>;` with no valid parse (`Expected expression, found ';'`). This
matches the `!self.had_line_terminator` guard tsv already applies to the sibling
type-argument sites (`typeof X` ⏎ `<T>`, `extends B` ⏎ `<T>`, postfix `B` ⏎ `[]`).
The same rule makes `type Y = B` ⏎ `<T>;` reject.

**acorn-typescript** instead *recovers*: it parses `let a: B` (type `B`, no
arguments) and then treats the leftover `<T>;` as a separate
`ExpressionStatement` whose expression is a floating `TSTypeParameterDeclaration`
— not real TypeScript, and rejected by both tsc and prettier.
`expected_svelte.json` records that two-statement recovery shape.

Because the canonical parser accepts this input, the rejection cannot be an
`input_invalid_*` fixture (which requires both parsers to reject). This
`tsv_rejects.txt` fixture pins the divergence from the other side: tsv rejects
(`tsv_rejects.txt` substring), while `expected_svelte.json` proves acorn still
accepts. The in-scope member direction — where the same rule splits `a: B` ⏎
`<T>(): C` into two members (both parsers agree) — is pinned by the sibling
[type_members/type_args_line_break](../../type_members/type_args_line_break/)
fixture.

**Upstream**: @sveltejs/acorn-typescript — `tsParseTypeReference` consumes
type arguments across a line break (no `hasPrecedingLineBreak` guard) and
recovers the trailing `<…>` as a floating type-parameter declaration.

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §TypeScript Corrections
(Type-reference type arguments after a line break).
