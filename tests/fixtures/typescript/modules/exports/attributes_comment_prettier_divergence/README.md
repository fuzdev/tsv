# attributes_comment_prettier_divergence

Comments in a re-export's attributes clause (`export … from … with {…}`, both the
`export { … } from` and `export * from` hosts) are preserved where the user placed
them — the re-export analog of the import `with_keyword_comment` /
`source_trailing_comment` divergences (the shared attribute-clause printer handles
all three hosts identically).

**Prettier** (`output_prettier.svelte`): keeps a source→`with` block comment in
place, relocates a `with`→`{` comment to before `with`, and pulls an after-`}`
comment *inside* the braces (trailing the last attribute):

```
export { a } from 'a' /* c1 */ with { type: 'json' };
export { b } from 'b' /* c2 */ with { type: 'json' };
export { c } from 'c' with { type: 'json' /* c3 */ };
export * from 'd' /* c4 */ with { type: 'json' };
export * from 'e' with { type: 'json' /* c5 */ };
```

**tsv**: preserves each comment where the user wrote it:

```
export { a } from 'a' /* c1 */ with { type: 'json' };
export { b } from 'b' with /* c2 */ { type: 'json' };
export { c } from 'c' with { type: 'json' } /* c3 */;
export * from 'd' with /* c4 */ { type: 'json' };
export * from 'e' with { type: 'json' } /* c5 */;
```

The source→`with` block comment (c1) is dual-stable. The `with`→`{` (c2, c4) and
after-`}` (c3, c5) comments diverge. Per Comment Position Philosophy.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
