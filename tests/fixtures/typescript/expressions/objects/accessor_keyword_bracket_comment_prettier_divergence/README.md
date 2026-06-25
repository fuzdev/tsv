# accessor keyword → computed-key bracket comment

Prettier relocates a comment between an accessor keyword (`get`/`set`) and the
`[` of a computed key to inside the brackets, before the key:
`get /* c */ [a]() {}` becomes `get [/* c */ a]() {}`.

tsv preserves the comment after the keyword, per the comment placement policy
(preserve user intent, don't relocate). The divergence is identical for the
interface, type-literal, and class accessor contexts (all share the
keyword→`[` bound); the fixture uses an object literal as the representative.

A comment *inside* the brackets (`get [/* c */ a]`) is a plain match — see the
non-divergence [accessor_computed_key_comment](../accessor_computed_key_comment/)
fixture, the regression guard for the in-bracket comment being emitted exactly
once (not duplicated onto the keyword).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
