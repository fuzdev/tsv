# modifier_pair_comment_prettier_divergence

Prettier coalesces comments interleaved between member modifier keywords after
the last modifier. Our formatter preserves the user's placement.

- Input: `static /* c1 */ readonly /* c2 */ p = 1;`
- Prettier: `static readonly /* c1 */ /* c2 */ p = 1;` (coalesces after modifiers)
- Ours: `static /* c1 */ readonly /* c2 */ p = 1;` (preserves interleaved)

Single-modifier comments (`static /* c */ p`) are preserved identically by both
formatters — see `../member_modifier_comment/`. Per comment placement policy,
we preserve user intent when prettier moves comments to different positions.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
