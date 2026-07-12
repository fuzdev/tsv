# property_key_es2015_ident_prettier_divergence

tsv unquotes any property key that is a valid **ES2015+** identifier; prettier only
unquotes keys valid under **ES5** (a frozen legacy table). An astral letter like `𐊧`
(U+102A7 CARIAN LETTER — a valid `ID_Start` in ES2015 but absent from ES5's table) is
the boundary case.

- **tsv**: `{ 𐊧: 1 }` — bare
- **Prettier**: `{ '𐊧': 1 }` — kept quoted

The bare form (`input`) is dual-stable — both formatters keep it — so the divergence
shows only when a quoted source (`prettier_variant_quoted`) is normalized: tsv unquotes
the key, prettier keeps it quoted (its own fixed point).

The rule is position-scoped and never over-unquotes:

- Object-literal keys, type-literal members, and interface members unquote.
- A class member key is never unquoted (stays a string literal in both formatters).
- A key that is not a valid identifier (`'0a'`) stays quoted in both.

## Reason

Design choice grounded in the spec. tsv's identifier check uses the Unicode
`ID_Start`/`ID_Continue` sets the ECMAScript grammar names (ecma262 §12.7 —
`IdentifierName :: IdentifierStart`, `IdentifierStart :: UnicodeIDStart`), so a key that
is a well-formed `IdentifierName` prints unquoted as a `LiteralPropertyName` (§13.2.5).
Prettier's ES5 table predates the Unicode-property definition and keeps these keys quoted.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §TypeScript.
