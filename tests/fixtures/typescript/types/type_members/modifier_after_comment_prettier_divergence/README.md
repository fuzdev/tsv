# modifier_after_comment_prettier_divergence

Prettier relocates comments after `?` in interface members:

**Property** (`a? /* c */ : number;`):
- Prettier: `a /* c */?: number;` (before `?`)
- Ours: `a? /* c */ : number;` (preserves after `?`)

**Method** (`b? /* c */(x): void;`):
- Prettier: `b?(/* c */ x): void;` (inside parens)
- Ours: `b? /* c */(x): void;` (preserves between `?` and `(`)

Both positions are dual-stable in our formatter. Per comment placement policy,
we preserve the user's original comment position.

Note: class properties with the same pattern (`d? /* c */: T = 1;`) are NOT
relocated by prettier — they match our output with a space before `:`.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
