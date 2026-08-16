# Exported `global` augmentation — Svelte Divergence

`export global { }` and `export declare global { }`. **tsc parses both** (empty
`parseDiagnostics`) and prettier formats both, byte-identically to tsv.
**acorn-typescript rejects both** — `'export declare' must be followed by an ambient
declaration.` — so this is a tsv over-acceptance against the shape oracle, and
`expected_svelte.json` pins that failure.

## Why tsv Differs

acorn gates its `shouldParseExportStatement` on `tokenIsTSDeclarationStart`, which
enumerates every sibling ambient head — `abstract`, `declare`, `enum`, `module`,
`namespace`, `interface`, `type` — and omits exactly one: `global`. Its own statement
path parses `global { }` happily (`tsParseExpressionStatement`), and it accepts
`export declare namespace N {}` and `export declare module 'a' {}`, the very same
production one name over. A verdict reached for every sibling and not for this one is
an oracle slip rather than a judgement — the call already made for the ambient `async`
signature in
[conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).

acorn-typescript is tsv's AST-**shape** target, not its correctness oracle; for validity
the oracle is tsc, and the accept test is prettier. Both take these, so tsv does too.

## The bodyless spellings are rejected, by all three

`export global;` and `export declare global;` are `input_invalid_*` here: with no body
the augmentation is not a declaration under tsc's `isDeclaration` (which demands `{`, an
identifier or `export` after `global`), so `export` is left with nothing to attach to and
prettier throws. acorn rejects them for its own reason. One check states it —
`Parser::require_exported_global_body`, asked at **both** export arms, because tsv used
to accept `export declare global { }` while rejecting `export global { }`, a split
neither oracle makes.

The restriction belongs to the `global` **name**, not to bodylessness: the shorthand's
string arm stays exportable in all three (`export declare module 'c';`, pinned by
[module_string_shorthand](../module_string_shorthand/)). Without `export`, the bodyless
`declare global;` is accepted and diverges from prettier instead —
[global_shorthand](../global_shorthand_prettier_divergence/).
