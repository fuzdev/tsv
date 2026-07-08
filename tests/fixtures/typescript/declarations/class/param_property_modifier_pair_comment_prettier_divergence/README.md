# param_property_modifier_pair_comment_prettier_divergence

Prettier coalesces comments interleaved between a parameter property's modifier
keywords after the last modifier. Our formatter preserves the user's placement —
the same rule the class-body member printer applies (see
`../modifier_pair_comment_prettier_divergence/`).

- Input: `constructor(public /* c1 */ readonly /* c2 */ x) {}`
- Prettier: `constructor(public readonly /* c1 */ /* c2 */ x) {}` (coalesces after modifiers)
- Ours: `constructor(public /* c1 */ readonly /* c2 */ x) {}` (preserves interleaved)

A single comment after the last modifier (`readonly /* c */ x`) is preserved
identically by both formatters — see `../param_property_modifier_comment/`. Per
comment placement policy, we preserve user intent when prettier moves comments to
different positions.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
