# Import-type type arguments after a line break (`import('./a').B` ⏎ `<T>`) — Svelte Divergence

A type-argument list binds to the type that precedes it only when no line
terminator intervenes — TypeScript's `parseTypeArgumentsOfTypeReference` is
guarded by `!scanner.hasPrecedingLineBreak()`. A `TSImportType`'s qualifier is one
of the sites that rule covers, so `import('./a').B` ⏎ `<string>` is the type
`import('./a').B` followed by a separate `<string>`, not `import('./a').B<string>`.

**tsv** follows tsc: the type ends at the qualifier, leaving `<string>;` with no
valid parse (`Expected expression, found ';'`). tsc rejects with **TS1109
"Expression expected"**, and prettier — whose `typescript` parser is tsc —
rejects too.

This is the **same rule** the plain type reference already applies in tsv, pinned
by the sibling [type_args/line_break](../line_break_svelte_divergence/), whose
conformance entry names the sites the guard covers. The import type was the one
site that read its arguments directly instead, so it welded where its siblings
split; the rule is now asked once for both.

## Why tsv Differs

**acorn-typescript accepts**, welding across the break into the very
`TSImportType` it builds for the same-line spelling — `qualifier: B` plus a
`typeArguments` list — so the line terminator simply vanishes. It is the same
recovery it performs for the plain type reference, one node kind over.

acorn-typescript is tsv's AST-**shape** target, not its correctness oracle; for
validity the oracle is tsc, which rejects. Since the canonical parser accepts,
the rejection cannot be an `input_invalid_*` fixture (which requires both parsers
to reject), so it is pinned from the other side: `tsv_rejects.txt` proves tsv
rejects, `expected_svelte.json` proves acorn still accepts.

Per ecma262 §sec-comments a `MultiLineComment` containing a line terminator *is*
a `LineTerminator`, so `import('./a').B /*` ⏎ `*/ <string>` is the same rejection
wearing trivia.

## The boundary

Only the type-position gap is restricted. The **expression** type-argument sites
carry no such rule and stay accepted everywhere — `f` ⏎ `<string>()` and
`new C` ⏎ `<string>()` are valid in tsc, acorn and tsv alike — as does a heritage
clause's `extends B` ⏎ `<string>`, which tsc reads through
`parseExpressionWithTypeArguments`.

**Upstream**: @sveltejs/acorn-typescript — `tsParseImportType` consumes type
arguments across a line break, the same missing `hasPrecedingLineBreak` guard
already filed for `tsParseTypeReference`.

See [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).
