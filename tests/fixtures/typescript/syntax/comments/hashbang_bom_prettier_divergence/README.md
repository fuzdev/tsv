# hashbang_bom_prettier_divergence

A hashbang behind a UTF-8 byte-order mark: `<BOM>#!/usr/bin/env node`. A BOM is the file's
encoding mark, not its text — a UTF-8 decode strips exactly one leading BOM (WHATWG Encoding),
which is what Node does before it reads the hashbang, and what tsc's file reader does before it
parses — so the hashbang still stands at the start of the source text, where
`InputElementHashbangOrRegExp` reads it (ecma262 §Hashbang Comments). tsv's lexer reads the BOM
the same way: it skips one at byte 0 before the first token.

`prettier_variant_bom.ts` is the BOM-led spelling. Prettier strips the BOM, formats, and writes
it back, so the variant is its own fixed point there; tsv strips a BOM with nothing
load-bearing behind it, so the variant formats to `input.ts`, the hashbang kept.

The parse wire has no oracle for the variant: acorn-typescript reads its input as a string, takes
U+FEFF as `WhiteSpace`, and rejects the `#!` behind it (`Unexpected character '!'`), as tsc's
`createSourceFile` does on a string that still holds the BOM (TS18026, "'#!' can only be used
at the start of a file"). So the variant carries no `expected_prettier_variant_bom.json`; that
deliberate accept is cataloged in
[conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).
Inside a Svelte `<script>` a U+FEFF is content, not an encoding mark, and no hashbang follows it.

See [conformance_prettier.md §Whitespace: BOM Handling](../../../../../../docs/conformance_prettier.md#whitespace-bom-handling)
and the plain [bom](../../whitespace/bom_prettier_divergence/) fixture.
