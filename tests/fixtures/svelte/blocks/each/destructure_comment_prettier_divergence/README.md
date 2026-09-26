# destructure_comment_prettier_divergence

A comment placed **inside** a `{#each … as PATTERN}` destructuring binding pattern is
preserved where the author wrote it. prettier-plugin-svelte silently drops it.

tsv: `{#each items as { a = /* c */ 1 }}` (preserved)
Prettier: `{#each items as { a = 1 }}` (comment dropped)

Covered positions (single-line block comments, which leave the pattern inline): an object default value
(`{ a = /* c */ 1 }`), leading after `{` (`{ /* c */ b }`), trailing before `}`
(`{ c /* c */ }`), the rename `key:` → value gap (`{ d: /* c */ e }`), an array element
(`[f /* c */]`), a rest binding (`[.../* c */ rest]`), a nested object default
(`[{ g = /* c */ 1 }]`), a comment **inside a default value** that is itself an
object/array expression (`{ r = { s: /* c */ 1 } }`, `{ t = [/* c */ 1] }` — kept inline,
since prettier keeps default values inline even when wide), and one glued **before** such
a value or before a nested object/array **pattern** (`{ u = /* c */ { v: 1 } }`,
`{ x: /* c */ { y } }`, `[/* c */ { z }]` — the positions where the comment is *owned* by
the brace/bracket node, so the piece's own builder is the only thing that can print it). An empty pattern holding only a block comment prints it between
spaced braces (`{ /* c */ }`) or tight brackets (`[/* c */]`), as the twin prints it. These are the same canonical
positions tsv preserves for a regular TypeScript destructure (`const { a = /* c */ 1 } = x`).

`unformatted_ours_own_line.svelte` writes the default's, the rename value's and the empty
patterns' comments on their own lines: a block comment at an `=` or `:` gap, or alone in an
empty pattern, collapses onto the line as the twin collapses it (`{ a =⏎/* c */⏎1 }` →
`{ a = /* c */ 1 }`, `{⏎/* c */⏎}` → `{ /* c */ }`), since neither position is a list gap
the twin opens for an own-line comment.

One cell is a `//` line comment (`{ q = // c⏎1 }`): it breaks the pattern open, laid out
like the same pattern in a TypeScript declaration — the full matrix of that broken layout is
[destructure_own_line_comment](../destructure_own_line_comment_prettier_divergence/).

The wire is a **parser match**: canonical parses the binding pattern with its own acorn
parse and attaches each interior comment to the adjacent node as `leadingComments` /
`trailingComments`, and tsv reproduces that attachment from the same window (the
`{@const}` binding shares the builder). The keyed case pins the window's near edge — a
pattern comment belongs to the pattern, never to the `(key)` expression that follows it,
whose own parse begins at the `(`.

## Prettier divergence (formatter)

User comments are valuable and shouldn't be silently removed; they are syntactically valid
in these positions. prettier-plugin-svelte prints these binding patterns from a
comment-blind path and drops them. See
[conformance_prettier_svelte.md §Svelte: destructuring binding-pattern comments](../../../../../../docs/conformance_prettier_svelte.md#svelte-destructuring-binding-pattern-comments).

## Related

- [destructure_comment](../../await/destructure_comment_prettier_divergence/) — same divergence for `{#await … then}` / `{:then}` / `{:catch}` patterns
- [destructure_own_line_comment](../destructure_own_line_comment_prettier_divergence/) — the comments that break the pattern open: `//`, own-line and multi-line block
- [context_annotation_comment](../context_annotation_comment_prettier_divergence/) — the same verdict one token later, inside the binding's type annotation
- [expr_trailing](../../../syntax/comments/expr_trailing_prettier_divergence/) — same drop-vs-preserve family for trailing comments in template expressions
