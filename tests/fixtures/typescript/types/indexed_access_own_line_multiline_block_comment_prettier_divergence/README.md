# indexed_access_own_line_multiline_block_comment_prettier_divergence

An **own-line multiline** block comment in an indexed access's `[`→index gap hangs the
index on its own line, the comment kept inside the brackets where the author wrote it.
Inlining it would reflow the author's break, so the own-line placement carries signal the
inline form would drop — the single-slot-gap rule's hang case, shared with the prefix
operators ([type_operator_keyword_own_line_multiline_block_comment](../type_operator_keyword_own_line_multiline_block_comment_prettier_divergence/)).

Contrast the two shapes that *do* collapse in this same gap, because inlining them loses
nothing: a **single-line** block in any position
([indexed_access_own_line_block_comment](../indexed_access_own_line_block_comment_prettier_divergence/))
and a **glued** multiline block
([keyword_value_glued_multiline_block_comment](../keyword_value_glued_multiline_block_comment/)).

## Prettier relocates the comment out of the brackets — and corrupts the type

Prettier is non-idempotent here, and its second pass **changes what the code means**:

| pass | output |
| --- | --- |
| 1 | `type X = A[/* c⏎d */⏎K];` — hangs the index (equivalent to tsv's, modulo the `=`) |
| 2 | `type X = A /* c⏎d */[K];` — relocates the comment out before `[` |
| 3 | `type X = A;` + `/* c⏎d */ [K];` — **no longer an indexed access** |

Pass 2 is already the corruption; pass 3 merely re-prints the changed tree. A type's
index suffix may not follow a line break (TypeScript's `parsePostfixTypeOrHigher` stops at
`scanner.hasPrecedingLineBreak()`), and the comment's *interior* newline supplies one — so
`A /* c⏎d */[K]` parses as `type X = A;` followed by an `ArrayExpression` statement. The
canonical parser agrees:

- `type X = A /* c */[K];` → `TSTypeAliasDeclaration → TSIndexedAccessType(A, K)`
- `type X = A /* c⏎d */[K];` → `TSTypeAliasDeclaration → TSTypeReference(A)` **+** `ExpressionStatement → ArrayExpression`

This is what separates the multiline case from its single-line sibling, where the same
relocation is *safe* (no newline, no line break before `[`) and prettier's relocated form
is genuinely dual-stable — pinned there as `variant_own_line`. Here no such variant exists:
prettier's fixed point is not a form of this type at all, so the chain is pinned only by
`output_prettier.svelte` (its first pass) and the divergence is content-preservation, not a
position preference.

tsv keeps the comment after `[` and hangs the index, in one pass, stably. Per
[Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy),
tsv keeps the comment where the author wrote it relative to the index.

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
