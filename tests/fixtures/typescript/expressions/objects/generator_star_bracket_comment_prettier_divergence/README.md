# generator `*` → computed-key bracket comment

Prettier relocates a comment between the generator `*` marker and the `[` of a
computed key to inside the brackets, before the key:
`*/* c */ [a]() {}` becomes `*[/* c */ a]() {}`.

tsv preserves the comment after the star, per the comment placement policy
(preserve user intent, don't relocate). The divergence is identical for class
generator methods and for `async *`; the fixture uses an object literal with
both forms as the representative.

A comment *inside* the brackets (`*[/* c */ a]`) is a plain match — see the
non-divergence [generator_computed_key_comment](../generator_computed_key_comment/)
fixture, the regression guard for the in-bracket comment being emitted exactly
once (not duplicated onto the `*`). Same family as the accessor-keyword case,
[accessor_keyword_bracket_comment](../accessor_keyword_bracket_comment_prettier_divergence/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
