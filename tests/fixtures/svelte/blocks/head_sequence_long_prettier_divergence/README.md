# head_sequence_long

A block head whose expression is a comma **sequence** and which exceeds printWidth. tsv
wraps it at the commas, dangles the closing `}` (and the `{#each}` `as` clause) onto its
own line at the tag's base indent, and expands the body. Prettier never width-wraps a
block head, so it keeps the whole head inline past printWidth.

The operands take the **continuation indent**, the same shape the binary chain beside it in
[if/long](../if/long_prettier_divergence/) takes — `group([first, ",", indent([line,
rest…])])`, so an operand that breaks internally keeps its own lines at the base column.
A sequence's own prettier geometry is the flush `group(join([",", line]))`, and tsv keeps
that at every braced head prettier width-wraps (`{(a, b)}`, `{@html (a, b)}`,
`attr={(a, b)}` — all matching). A block head is the one head prettier never wraps at all,
so the wrap is tsv's own and carries tsv's own geometry rather than a shape inherited from
a position prettier answered.

Boundary shapes covered: head + body fitting (fully inline, both formatters agree); the
head alone at exactly 100 (stays flat, only the body expands — the middle zone); the head
alone at 101 (operands indent, `}` dangles); an `{#each}` head, where the `as item` clause
follows the wrapped head on the dangled line; and an `{#each}` **key**, a head of its own
inside the head — same indent, its 100/101 boundary its own, and its own `)` dropping to
base the way a block head's `}` does (that closer rule is the key's, not the sequence's —
[each/key_long](../each/key_long_prettier_divergence/) owns it for every key expression
kind).

## Reason

See [conformance_prettier_svelte.md §Svelte: Blocks](../../../../../docs/conformance_prettier_svelte.md#svelte-blocks)
for the head-wrap + `}` dangle + clause-hug + body-expand + middle-zone model, and
[conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy)
for why tsv treats the width as a limit where prettier lets a block head overflow.

## Related

- [if/long](../if/long_prettier_divergence/) · [each/long](../each/long_prettier_divergence/) — the same divergence for a binary / member chain / call head
- [each/key_long](../each/key_long_prettier_divergence/) — the key's own closer rule, across every key expression kind
- [expressions/sequence/long](../../../typescript/expressions/sequence/long/) — the sequence's own layouts in `<script>`, where prettier agrees (this indent is prettier's own `ExpressionStatement` / `for`-head arm, borrowed for a position prettier leaves unanswered)
