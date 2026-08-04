# Divergence: `await`→`using` keyword-interior comment (preserve)

A block comment inside the `await using` keyword (`await /* c */ using a = f();`), including the
for-await-of head. tsv keeps it after `await`; prettier **relocates** it past `using` onto the
binding.

```ts
// tsv (preserve)                // prettier (relocate past the keyword)
await /* c */ using a = f();     await using /* c */ a = f();
```

**Why tsv preserves:** relocating **collapses a distinction**. With comments on both sides of
`using`, prettier lands both in one place — `await /* c2 */ using /* c3 */ b` becomes
`await using /* c2 */ /* c3 */ b` — so "before `using`" and "after `using`" become
indistinguishable. The text survives; the association does not. tsv keeps each on its authored
side. A keyword is not a *pure separator*, the one sanctioned reason to trail.

Only the **block** form exists: a comment containing a newline *is* a `LineTerminator` (ecma262
§sec-comments) and `await [no LT] using` demotes, so `await /* c⏎ */ using y = f()` is correctly
not a declaration in either formatter.

## Parser divergence

Svelte's parser (acorn-typescript) rejects `using` / `await using` outright — the pre-existing
divergence pinned by [await_svelte_divergence](../await_svelte_divergence/), not anything about the
comment. Hence `expected_ours.json` + `expected_svelte.json` and the `_svelte_prettier_divergence`
suffix. See [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).

See [conformance_prettier_ts_comments.md §Comments inside a multi-word keyword](../../../../../../docs/conformance_prettier_ts_comments.md#comments-inside-a-multi-word-keyword)
and [§Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
