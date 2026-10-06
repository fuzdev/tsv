# class_field_key_quoted_prettier_divergence

A quoted class **field** key keeps its quotes wherever a class is printed
([field_key_quoted](../../../typescript/declarations/class/field_key_quoted/)), a class
expression in a template expression included. Prettier unquotes it there.

- **tsv**: `'a': string;`, `'b' = 1;`, `static 'c' = 2;` — kept quoted, in a text-position
  expression and in a `{@const}` alike
- **Prettier**: `a: string;`, `b = 1;`, `static c = 2;` — unquoted

Both unquote the method key beside them (`'m'() {}` → `m() {}`), and the quote style still
normalizes (`unformatted_ours_double_quotes`).

## Reason

◆prettier_bug. Prettier keeps a class field's quotes because TypeScript's
`strictPropertyInitialization` check exempts a string-named field and not an identifier-named
one, and it states that guard by parser name (`typescript`, `babel-ts`).
prettier-plugin-svelte formats a template expression through parsers of its own names
(`svelteExpressionParser`, `svelteTSExpressionParser` and their statement twins), which the
guard does not list, so every field key unquotes there — in a `lang="ts"` component too,
where `'a': string;` checks clean under `strict` and the bare `a: string;` is `TS2564: Property
'a' has no initializer and is not definitely assigned in the constructor`. The same class
written in the `<script>` keeps its quotes in both formatters.

See [conformance_prettier_ts.md](../../../../../docs/conformance_prettier_ts.md) §TypeScript.
