# property_modifier_comment_prettier_divergence

Prettier relocates a comment between a property name and its `?` modifier
(no type annotation) to before the modifier:

- Input: `a? /* c1 */ = 1;`
- Prettier: `a /* c1 */? = 1;`
- Ours: `a? /* c1 */ = 1;` (preserves the user's position)

Per comment placement policy, we preserve the user's original comment position.
The before-modifier form is dual-stable in both formatters
(`variant_before_modifier`, identical to prettier's relocation target); only the
after-modifier form (input) diverges — prettier moves it, tsv keeps it.

An **optional** (`?`) property with an initializer and no type annotation
(`a? = 1`) is valid TypeScript, so this divergence stays live. The parallel
**definite** (`!`) property (`b! = 1`) is *not* valid — a definite-assignment
assertion with no type annotation is a TypeScript early error (TS1264), which
Prettier 3.9.6 rejects at parse time — so that case is the `prettier_rejects`
fixture [property_definite_comment](../property_definite_comment_prettier_divergence/)
rather than a relocation divergence here. (Prettier ≤3.9.5 accepted `b!` and
relocated its comment too; this fixture covered both modifiers until 3.9.6
tightened the parser.)

When a type annotation is present (e.g. `a /* c */?: number;` or
`c! /* c */: number;`) both formatters agree and preserve the comment — that
case is the regular `property_modifier_type_comment` fixture (no
`_prettier_divergence`).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
