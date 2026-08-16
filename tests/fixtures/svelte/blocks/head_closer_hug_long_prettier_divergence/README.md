# head_closer_hug_long_prettier_divergence

A block head that wraps and whose expression **ends on its own closing delimiter**, dedented
to the tag's base indent. That last line already starts with a closer, so the tag's `}` (and
any `as item` clause) continues on it — `}}`, `] as x}`, `)}` — instead of opening a second
closer line under the first. Prettier never width-wraps a block head, so it keeps the whole
head inline past printWidth and never faces the question.

This is the *hug* half of the head-closer rule, and the question it asks is "does the broken
form end on a dedented delimiter of its own?". Every bracket-delimited literal does: an
object, an array, a function or class expression, a block-bodied arrow — alongside the single
call / `new` whose arguments wrapped, and `import(…)`, which owns its parens outright. A
prefix operator is transparent to it (`!fn(⏎…⏎)`, `await fn(⏎…⏎)` still end on the call's
`)`), as are an angle-bracket cast and a trailing non-null `!`, which glues to that `)`.

⚠️ Under-approximating is safe here and over-approximating is not — a false negative costs
one extra closer line (verbose, idempotent, reparses), a false positive glues the tag's
closer onto a line of *content*. So a wrapper recurses on its operand even where it
sometimes synthesizes a paren shell of its own, and a **JSDoc cast is deliberately off the
list** despite owning its parens: the shape where a *flat* cast sits under a multi-line
comment that broke the head for it is already pinned at
[head_jsdoc_cast_multiline_comment](../head_jsdoc_cast_multiline_comment_svelte_prettier_divergence/).

Shapes covered: an object-literal head; an array-literal head, where the `as x` clause joins
the dedented `]`; a block-bodied arrow head; a prefix operator over a wrapped call; an
`{#each}` **key**, which answers the same question one level in; a **control** — a binary
head, which ends on an operand one indent in, so its `}` still drops to its own line; and the
two paren-owning kinds, `import(…)` and a prefix `await`.

## Reason

See [conformance_prettier_svelte.md §Svelte: Blocks](../../../../../docs/conformance_prettier_svelte.md#svelte-blocks)
for the head-wrap + `}` dangle + closer-hug model, and
[conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy)
for why tsv treats the width as a limit where prettier lets a block head overflow.

## Related

- [if/long](../if/long_prettier_divergence/) · [each/long](../each/long_prettier_divergence/) — the call/`new` hug and the dangle it is the exception to, per block head
- [each/key_long](../each/key_long_prettier_divergence/) — the same closer rule asked of an `{#each}` key
- [head_sequence_long](../head_sequence_long_prettier_divergence/) — a sequence, whose `)` is glued to the last operand one indent in and so does **not** hug
