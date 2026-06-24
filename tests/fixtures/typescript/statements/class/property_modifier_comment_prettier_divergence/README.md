# property_modifier_comment_prettier_divergence

Prettier relocates a comment between a property name and its `?`/`!` modifier
(no type annotation) to before the modifier:

- Input: `a? /* c1 */ = 1;` / `b! /* c2 */ = 1;`
- Prettier: `a /* c1 */? = 1;` / `b /* c2 */! = 1;`
- Ours: preserves the user's position (`a? /* c1 */`, `b! /* c2 */`)

Per comment placement policy, we preserve the user's original comment position.
The before-modifier form is dual-stable in both formatters
(`variant_before_modifier`, identical to prettier's relocation target); only the
after-modifier form (input) diverges — prettier moves it, tsv keeps it.

Note: when a type annotation is present (e.g., `a /* c */?: number = 1;` or
`c! /* c */: number = 1;`), both formatters preserve the comment on either side
of the modifier — no divergence — so that case is the regular
`property_modifier_type_comment` fixture (no `_prettier_divergence`). The
divergence here is specific to the **annotation-free** property, where prettier
relocates the after-modifier comment.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
