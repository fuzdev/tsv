# body_conditional_operand_paren_trailing_block_comment_prettier_divergence

A block comment written last inside the pair an arrow's conditional body keeps around a `??`
operand. The arrow prints its conditional body in a pair of its own, and the `??` operand
keeps its pair inside it. A redundant inner shell (`x = () => c ? d : (a ?? (b /* c */));`)
reaches `input` in one pass in both formatters.

**tsv**: keeps the comment inside the `??` operand's pair, where the author wrote it, in one
pass:

```ts
x = () => (c ? d : (a ?? b /* c */));
```

**Prettier**: moves the comment out past the operand's `)` (`output_prettier.svelte`):

```ts
x = () => (c ? d : (a ?? b) /* c */);
```

The pair is in the output, so the author's position is representable and tsv holds it; no
statement terminator reaches past the pair the arrow adds around its body.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment Position Philosophy and
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
