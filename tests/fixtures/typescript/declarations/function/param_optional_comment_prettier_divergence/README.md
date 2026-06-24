# param_optional_comment_prettier_divergence

Prettier relocates comments from before `?` to after `?` in function parameter
declarations. Our formatter preserves the user's placement (before `?`).

- Input: `function fn(a /* c */?: number) {}`
- Prettier: `function fn(a? /* c */ : number) {}` (moves after `?`)
- Ours: `function fn(a /* c */?: number) {}` (preserves before `?`)

Both positions are dual-stable in our formatter (`variant_after_question.svelte`
records prettier's after-`?` form, which our formatter also keeps stable). Per
comment placement policy, we preserve user intent when prettier moves comments
to different positions. Same pattern applies to arrow-function params.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
