# readonly index-signature keyword comment

Prettier relocates a comment between the `readonly` keyword and the `[` of an
index signature to inside the brackets, before the key:
`readonly /* c */ [k: string]: A` becomes `readonly [/* c */ k: string]: A`.

tsv preserves the comment after the keyword, per the comment placement policy
(preserve user intent, don't relocate). The formatter divergence is identical
for a type-literal member; the fixture uses an interface because a type-literal
`readonly` index signature with this comment exposes an unrelated parser
comment-attachment difference from acorn-typescript.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
