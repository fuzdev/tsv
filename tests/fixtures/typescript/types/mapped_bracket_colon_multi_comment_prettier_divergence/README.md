# Divergence: multiple line comments between mapped-type `]` and the value `:`

Two line comments in a mapped type's `]`→value-`:` gap
(`[K in keyof T] // c1⏎// c2⏎: V`). The single-comment case is
[mapped_bracket_colon_line_comment](../mapped_bracket_colon_line_comment_prettier_divergence/);
this is the multi-comment extension, where each comment must stay a **separate**
node.

tsv keeps every comment where the author wrote it — the first trails `]` on its
line, the second keeps its own line — then drops the value `:` to a continuation
line **indented one level** (uniform forced-continuation indent):

```ts
[K in keyof T] // c1
	// c2
	: V;
```

## No prettier oracle — prettier never converges

Prettier has **no stable form** for two line comments in this gap. It oscillates
forever: one pass relocates the first comment inside the broken brackets and
leaves the second trailing the value `:` (`]: // c2⏎V`), the next pulls the
second inside the brackets too (`// c2⏎]: V`) and then pushes it back out —
flipping on every pass, so there is no `output_prettier.svelte` to anchor
against. This is recorded with a `prettier_nonconvergent.txt` marker,
live-verified by the validator (rule F5). tsv, by contrast, is stable and
lossless on the same input. The index-signature sibling oscillates the same way
([index_signature_bracket_colon_multi_comment](../type_members/index_signature_bracket_colon_multi_comment_prettier_divergence/)).

## The bug this guards

Without a per-comment line break, the second line comment is **swallowed** by
the first: emitting each comment with only a leading space renders `// c1` and
`// c2` on one line (`// c1 // c2`) — the `// c2` becomes text inside `// c1`,
content loss and non-idempotent. Each comment must be emitted on its own line.
See
[conformance_prettier.md](../../../../../docs/conformance_prettier.md)
§Comment relocation, §Uniform Forced-Continuation Indent, and §Comment Position
Philosophy.
