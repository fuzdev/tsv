# jest_each_table_prettier_divergence

Prettier special-cases `` describe.each` `` / `` test.each` `` / `` it.each` ``
tagged templates (`template-literal.js` `isJestEachTemplateLiteral` /
`printJestEachTemplateLiteral`): it parses the `` `…` `` body as a `|`-separated
table and **re-aligns** it, padding every cell to its column's widest entry.
`output_prettier.svelte` shows the misaligned `input.svelte` table padded into
aligned columns.

tsv does **not** special-case jest-each. A tagged template's body is preserved
verbatim like any other template's quasi text — the `${…}` interpolations still
format normally (atomized), but the surrounding cell spacing is kept exactly as
authored. So the table in `input.svelte` stays misaligned; only Prettier pads it.

## Reason

Deliberate tsv choice: no jest-specific magic. Prettier's table re-alignment is a
name-triggered special case — the tag must be `describe` / `test` / `it`
(optionally `.only` / `.skip`) with an `.each` member. tsv treats every tagged
template uniformly, formatting the body as authored text rather than detecting a
testing framework by identifier. The interpolations format identically in both;
only the cell padding differs.

See [conformance_prettier.md](../../../../../../../docs/conformance_prettier.md) §TypeScript: Template Literals.
