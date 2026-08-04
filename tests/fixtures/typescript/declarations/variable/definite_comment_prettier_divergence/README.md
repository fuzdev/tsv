# definite_comment_prettier_divergence

Prettier relocates comments from before `!` to after `!` in variable definite
assignment declarations. Our formatter preserves the user's placement (before `!`).

- Input: `let a /* c */!: number;`
- Prettier: `let a! /* c */ : number;` (moves after `!`)
- Ours: `let a /* c */!: number;` (preserves before `!`)

Both positions are dual-stable in our formatter (`variant_after_bang.svelte`
records prettier's after-`!` form, which our formatter also keeps stable). Per
comment placement policy, we preserve user intent when prettier moves comments
to different positions.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
