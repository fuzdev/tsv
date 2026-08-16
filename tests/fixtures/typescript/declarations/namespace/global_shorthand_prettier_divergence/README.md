# Divergence: bodyless `declare global` (one declaration, not two statements)

`declare global;` — the `global` arm of the ambient-module production taken without a body. tsv
reads it as one bodyless `TSModuleDeclaration` (`global: true`, no `body`), the acorn shape and the
exact shape tsv already emits for the string-literal arm (`declare module 'a';`). Prettier reads it
as **two identifier expression statements** and prints `declare;⏎global;`.

```ts
// tsv (one declaration)   // prettier (two statements)
declare global;            declare;
                           global;
```

**Both oracles' module-declaration production admits it.** acorn-typescript's
`tsParseAmbientExternalModuleDeclaration` takes `global` as the name and then `if (braceL) body
else semicolon()` — the same branch the string arm takes. tsc's `parseAmbientExternalModuleDeclaration`
is written identically (`parseIdentifier()` + `GlobalAugmentation` flag, then `parseModuleBlock()`
or `parseSemicolon()`). Only tsc's *statement-level routing* diverts: `isDeclaration`'s
`GlobalKeyword` case requires `{`, an identifier, or `export` after `global`, so a `;` sends the
whole thing down the expression-statement path instead.

**Following prettier there is not available to tsv.** Its split needs a semicolon between `declare`
and `global`, two words on **one line** with no `LineTerminator` between them, and none of
ECMAScript's three ASI conditions
([§sec-rules-of-automatic-semicolon-insertion](https://tc39.es/ecma262/#sec-rules-of-automatic-semicolon-insertion))
admits one. It is the same insertion tsv already refuses at `declare bar⏎function f(): void;` —
where tsv *rejects* rather than diverging, because acorn rejects too and no reading is left. Here
acorn supplies a reading, so tsv takes it rather than refusing to format the file at all. Prettier
is not an independent witness either way: its `typescript` parser is tsc's.

The last two cases are the null control on the other half of the rule: **without** `declare`, a
bodyless `global` stays an ordinary expression statement in tsv, acorn *and* tsc — the bare arm
requires a `{` before it is a declaration at all (acorn's `tsParseExpressionStatement`, tsv's own
peek in `parse_statement`). That asymmetry is the oracles', not tsv's, and it is what keeps the
change confined to the `declare` route. Behind `export` the shorthand is rejected by all three
oracles — `export` is left with nothing to attach to — and the whole `export` × `global` matrix
lives in [global_export](../global_export_svelte_divergence/).

The `/* c */` case pins the name→`;` gap the shorthand newly opens; it is the emitter the
string-literal arm already uses (`push_semicolon_with_gap_comments`), shared so neither arm can
drift — see [module_string_shorthand_comment](../module_string_shorthand_comment/).

See [conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
for the catalog entry, and
[§tsv rejects what prettier formats](../../../../../../docs/conformance_prettier_ts.md#tsv-rejects-what-prettier-formats)
for the ASI argument it shares.
