# Divergence: multiple line comments between index-signature `]` and the value `:`

Two line comments in the `]`→value-`:` gap
(`[k: string] // a⏎// b⏎: number`). The single-comment case is
[index_signature_bracket_colon_line_comment](../index_signature_bracket_colon_line_comment_prettier_divergence/);
this is the multi-comment extension, where each comment must stay a **separate**
node.

tsv keeps every comment where the author wrote it — the first trails `]` on its
line, the second keeps its own line — then drops the value `:` to a continuation
line **indented one level** (uniform forced-continuation indent):

```ts
[k: string] // a
	// b
	: number;
```

## No prettier oracle — prettier never converges

Prettier has **no stable form** for two line comments in this gap. It oscillates
forever between pulling the second comment inside the brackets
(`…// b⏎]: number`) and leaving it after `]` (`] // b⏎: number`), flipping on
every pass — so there is no `output_prettier.svelte` to anchor against. This is
recorded with a `prettier_nonconvergent.txt` marker, live-verified by the
validator (rule F5). tsv, by contrast, is stable and lossless on the same input.

## The bug this guards

Without a per-comment line break, the second line comment is **swallowed** by the
first: the gap loop emits each comment with only a leading space, so `// a` and
`// b` render on one line (`// a // b`) — the `// b` becomes text inside `// a`,
content loss and non-idempotent. Each comment must be emitted on its own line, the
same rule the key-type→`]` gap already follows. A **block** comment in this gap
stays inline in both formatters (covered by
[index_signature_bracket_colon_comment](../index_signature_bracket_colon_comment/));
only line comments differ — a `//` runs to EOL, so the value `:` must drop to its
own line. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation, §Uniform Forced-Continuation Indent, and §Comment Position
Philosophy.
