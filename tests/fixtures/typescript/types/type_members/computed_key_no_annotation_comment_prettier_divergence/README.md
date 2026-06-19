# computed_key_no_annotation_comment_prettier_divergence

A block comment after a **computed** property-signature key that has **no type
annotation** (`[Symbol.iterator] /* c */;`). Prettier relocates it inside the
brackets, before `]`; tsv preserves it after `]`.

**Interface** (`[Symbol.iterator] /* c */;`):

- Prettier: `[Symbol.iterator /* c */]` (inside the brackets)
- Ours: `[Symbol.iterator] /* c */;` (preserves after `]`)

Same relocation family as the other [computed key after `]`](../computed_key_bracket_comment_prettier_divergence/)
divergences — prettier pulls a post-`]` comment inside the brackets — but in the
no-annotation gap (`]`→`;` instead of `]`→`:`). Both positions are dual-stable.
Per comment placement policy, we preserve the user's original position.

(The type-literal counterpart isn't covered: tsv's parser rejects a computed
property signature with no annotation in a type literal — an unrelated parser
gap, tracked separately.)

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
