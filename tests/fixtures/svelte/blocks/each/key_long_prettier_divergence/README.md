# key_long_prettier_divergence

An `{#each}` **key** whose expression exceeds printWidth. The key is a head of its own
inside the block head — `(` … `)` after the `as` clause — and it takes the block head's
own closer rule: when the key wraps, its `)` drops to its own line at the tag's base
indent and the `}` follows it there (`⏎)}`), rather than hugging the last content line.
Prettier never width-wraps any part of a block head, so it keeps the whole key inline
past printWidth.

The rule and its hug exception are the head's, asked of the key: an expression whose broken
form already ends on a dedented closer — a single call / `new` whose arguments wrapped, or
any bracket-delimited literal — puts the key's `)` + `}` on that line (`))}`) instead of
opening a second closer line. See
[head_closer_hug_long](../../head_closer_hug_long_prettier_divergence/) for that half.

⚠️ The key is a head **because the `as` clause makes it a sibling of the head expression**.
In the degenerate no-`as` form (`{#each xs, i (key)}` — Svelte's parser accepts it, its
compiler rejects it) there is no clause, so the key is concatenated into the head
expression's own doc: a key that breaks there breaks the head group, and the tag's own `}`
is already the closer drop the rule asks for. Giving the key a second one would stack two
closer lines, so it keeps the plain hugged `)`.

Shapes covered: a key head at exactly 100 (stays flat, only the body expands — the middle
zone); the same at 101 (wraps, `)` drops); a member chain (wraps on prettier's 3+-group
rule, same drop); a single call whose arguments wrap (the `))}` hug); and the degenerate
no-`as` form, whose one dangling closer is the tag's.

## Reason

See [conformance_prettier_svelte.md §Svelte: Blocks](../../../../../../docs/conformance_prettier_svelte.md#svelte-blocks)
for the head-wrap + `}` dangle + clause-hug model this extends from the head expression to
the key, and
[conformance_prettier.md §Print Width Philosophy](../../../../../../docs/conformance_prettier.md#print-width-philosophy)
for why tsv treats the width as a limit where prettier lets a block head overflow.

## Related

- [each/long](../long_prettier_divergence/) — the same closer rule for the `{#each}` head expression, where the clause + `}` drop together
- [head_closer_hug_long](../../head_closer_hug_long_prettier_divergence/) — the hug half: which expressions end on a dedented closer, at a head and at a key
- [blocks/head_sequence_long](../../head_sequence_long_prettier_divergence/) — a **sequence** key, whose operands indent and whose `)` drops the same way
- [if/long](../../if/long_prettier_divergence/) · [key/long](../../key/long_prettier_divergence/) · [await/long](../../await/long_prettier_divergence/) — the same divergence per block head
