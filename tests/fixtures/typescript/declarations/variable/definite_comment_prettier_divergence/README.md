# definite_comment_prettier_divergence

Prettier relocates comments from before `!` to after `!` in variable definite
assignment declarations. Our formatter preserves the user's placement (before `!`).

- Input: `let a /* c */!: number;`
- Prettier: `let a! /* c */ : number;` (moves after `!`)
- Ours: `let a /* c */!: number;` (preserves before `!`)

Same relocation without a type annotation (`let c /* c */! = 1;`, `let d /* c */!;`):
prettier moves the comment after `!`; ours preserves it before `!`.

Both positions are dual-stable in our formatter. Per comment placement policy,
we preserve user intent when prettier moves comments to different positions.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
