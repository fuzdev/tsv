# optional_marker_comment_prettier_divergence

Prettier relocates a block comment written after the optional `?` marker in a
class method to *before* `?`; tsv preserves the user's placement (after `?`).

**Method** (`m? /* c */(x: number): void {}`):

- Prettier: `m /* c */?(x: number): void {}` (moves before `?`)
- Ours: `m? /* c */(x: number): void {}` (preserves between `?` and `(`)

Note the asymmetry with interface/type-literal method signatures: prettier moves
the comment *into the parens* there (`m?(/* c */ x): void`), but for a class
method — which has a body — it moves *before* `?`, regardless of params.

Both positions are dual-stable in our formatter. Per the comment-position
policy, we preserve the user's original comment position.

A comment written *before* `?` (`m /* c */?(x): void {}`) is a match in both
formatters — see `declarations/class/optional_marker_before_comment`. A class
*property* with the comment after `?` and a type annotation (`a? /* c */ : T`)
is also a match (prettier preserves it) — see
`statements/class/property_modifier_type_comment`.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
