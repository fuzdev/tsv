# head_sequence_long

A block head whose expression is a comma **sequence** and which exceeds printWidth. tsv
wraps it at the commas, dangles the closing `}` (and the `{#each}` `as` clause) onto its
own line at the tag's base indent, and expands the body. Prettier never width-wraps a
block head, so it keeps the whole head inline past printWidth.

The operands wrap **flush** rather than continuation-indented, which is the sequence's own
prettier geometry (`group(join([",", line]))` — no indent), unlike the binary chain beside
it in [if/long](../if/long_prettier_divergence/), which reaches a continuation indent
through prettier's binaryish parent rule. The divergence here is only about *whether* the
head breaks, so each node kind keeps the shape prettier gives it.

Boundary shapes covered: head + body fitting (fully inline, both formatters agree); the
head alone at exactly 100 (stays flat, only the body expands — the middle zone); the head
alone at 101 (wraps at the comma, `}` dangles); and an `{#each}` head, where the `as item`
clause follows the wrapped head on the dangled line.

## Reason

See [conformance_prettier_svelte.md §Svelte: Blocks](../../../../../docs/conformance_prettier_svelte.md#svelte-blocks)
for the head-wrap + `}` dangle + clause-hug + body-expand + middle-zone model, and
[conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy)
for why tsv treats the width as a limit where prettier lets a block head overflow.

## Related

- [if/long](../if/long_prettier_divergence/) · [each/long](../each/long_prettier_divergence/) — the same divergence for a binary / member chain / call head
- [expressions/sequence/long](../../../typescript/expressions/sequence/long/) — the sequence's own layouts in `<script>`, where prettier agrees
