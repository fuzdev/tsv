# field_key_unquote_prettier_divergence

tsv unquotes a **class field** key that is a valid identifier, exactly as it unquotes an
object / type-literal / interface key (`'x' = 1` → `x = 1`). Prettier unquotes class
*method* and *accessor* keys but leaves class *field* keys quoted — its own inconsistency.

- **tsv**: `x = 1`, `y`, `fn = () => 1` — unquoted; the rule holds across every field form:
  type-annotated (`count: number`), optional (`optional?`), and `static total`
- **Prettier**: `'x' = 1`, `'y'`, `'fn' = () => 1`, `'count': number`, `'optional'?`,
  `static 'total'` — all kept quoted

The bare form (`input`) is dual-stable — both formatters keep it — so the divergence shows
only when a quoted source (`prettier_variant_quoted`) is normalized: tsv unquotes the valid-
identifier field key, prettier keeps it quoted (its own fixed point). **Non-identifier keys
stay quoted in both** (`'0a'`, `'x-y'`, numeric `'0'`) — the contrast cases proving tsv never
over-unquotes — as do escape-bearing keys. A reserved word is a valid member-key identifier
(ecma262 `PropertyName :: LiteralPropertyName :: IdentifierName`), so `'in' = 1` → `in = 1`
too. Class *methods* / *accessors* / *constructor* unquote in both (see
[member_key_unquote](../member_key_unquote/) — a non-divergence fixture where tsv matches
prettier).

## Reason

Design choice for consistency, and more uniform than prettier. tsv applies one
"unquote a valid-identifier key" rule at every non-computed key position — object
properties, type-literal / interface members, and **every** class member (method,
accessor, static, and field). Prettier unquotes class method/accessor keys but keeps
class field keys quoted, so an object property `{ 'x': 1 }` → `{ x: 1 }` yet a class field
`'x' = 1` stays quoted under prettier; tsv removes that inconsistency. Unquoting is always
meaning-preserving here — a valid-identifier field key names the same field either way.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §TypeScript.
