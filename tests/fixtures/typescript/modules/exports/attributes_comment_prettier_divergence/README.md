# attributes_comment_prettier_divergence

Comments in a re-export's attributes clause (`export … from … with {…}`, both the
`export { … } from` and `export * from` hosts) are preserved where the user placed
them — the re-export analog of the import `with_keyword_comment` /
`source_trailing_comment` divergences (the shared attribute-clause printer handles
all three hosts identically).

**Prettier** (`output_prettier.svelte`): keeps a source→`with` block comment in
place (c1), relocates a `with`→`{` comment to before `with` (c2, c4), and trails an
after-`}` comment past the `;` (c3, c5):

```
export { a } from 'a' /* c1 */ with { type: 'json' };
export { b } from 'b' /* c2 */ with { type: 'json' };
export { c } from 'c' with { type: 'json' }; /* c3 */
export * from 'd' /* c4 */ with { type: 'json' };
export * from 'e' with { type: 'json' }; /* c5 */
```

**tsv**: preserves each comment where the user wrote it, except it trails an
after-`}` comment past the `;` to match prettier (c3, c5 — the lossless
trail-past-a-separator carve-out):

```
export { a } from 'a' /* c1 */ with { type: 'json' };
export { b } from 'b' with /* c2 */ { type: 'json' };
export { c } from 'c' with { type: 'json' }; /* c3 */
export * from 'd' with /* c4 */ { type: 'json' };
export * from 'e' with { type: 'json' }; /* c5 */
```

The source→`with` block comment (c1) and the after-`}` comments (c3, c5) agree
between both formatters. Only the `with`→`{` comments (c2, c4) diverge: prettier
relocates them before `with`, tsv keeps them after `with` per Comment Position
Philosophy.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
