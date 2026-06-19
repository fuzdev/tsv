# property_signature_no_annotation_optional_comment_prettier_divergence

A block comment after the optional `?` of a property signature that has **no
type annotation** (`a? /* c */;`). Prettier relocates it before `?`; tsv
preserves it after the marker — the same relocation family as
[modifier_after_comment](../modifier_after_comment_prettier_divergence/), but in
the no-annotation gap (`?`→`;` instead of `?`→`:`).

**Interface and type-literal** (`a? /* c */;`):

- Prettier: `a /* c */?;` (before `?`)
- Ours: `a? /* c */;` (preserves after `?`)

Both positions are dual-stable. Per comment placement policy, we preserve the
user's original comment position. (Without this handling the comment was
dropped entirely — a content-loss bug; preserving it is also the fix.)

The non-optional and `readonly` no-annotation cases are a plain match (prettier
keeps `a /* c */;` in place too) — see
[property_signature_no_annotation_comment](../property_signature_no_annotation_comment/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
