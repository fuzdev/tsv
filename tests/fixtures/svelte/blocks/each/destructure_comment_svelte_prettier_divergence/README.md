# destructure_comment_svelte_prettier_divergence

A comment placed **inside** a `{#each … as PATTERN}` destructuring binding pattern is
preserved where the author wrote it. prettier-plugin-svelte silently drops it.

tsv: `{#each items as { a = /* c */ 1 }}` (preserved)
Prettier: `{#each items as { a = 1 }}` (comment dropped)

Covered positions (all block comments, pattern stays inline): an object default value
(`{ a = /* c */ 1 }`), leading after `{` (`{ /* c */ b }`), trailing before `}`
(`{ c /* c */ }`), the rename `key:` → value gap (`{ d: /* c */ e }`), an array element
(`[f /* c */]`), a rest binding (`[.../* c */ rest]`), a nested object default
(`[{ g = /* c */ 1 }]`), and a comment **inside a default value** that is itself an
object/array expression (`{ r = { s: /* c */ 1 } }`, `{ t = [/* c */ 1] }` — kept inline,
since prettier keeps default values inline even when wide). These are the same canonical
positions tsv preserves for a regular TypeScript destructure (`const { a = /* c */ 1 } = x`).

## Svelte divergence (parser)

The binding pattern is parsed by acorn (the each `context`), which attaches the comment
to the adjacent AST node as `leadingComments` / `trailingComments`. tsv uses its detached
comment model — every comment lives once in the root `comments` array, never duplicated
onto nodes — so `expected_ours.json` omits those attachments that `expected_svelte.json`
carries. The set of distinct comments is identical and `ast_diff` confirms semantic
equivalence; the formatter (which locates comments by position) is unaffected. Same family
as the other acorn comment-attachment divergences. See
[conformance_svelte.md §Comment Attachment Differences](../../../../../../docs/conformance_svelte.md#comment-attachment-differences).

## Prettier divergence (formatter)

User comments are valuable and shouldn't be silently removed; they are syntactically valid
in these positions. prettier-plugin-svelte prints these binding patterns from a
comment-blind path and drops them. See
[conformance_prettier.md §Svelte: destructuring binding-pattern comments](../../../../../../docs/conformance_prettier.md#svelte-destructuring-binding-pattern-comments).

## Related

- [destructure_comment](../../await/destructure_comment_svelte_prettier_divergence/) — same divergence for `{#await … then}` / `{:then}` / `{:catch}` patterns
- [expr_trailing](../../../syntax/comments/expr_trailing_prettier_divergence/) — same drop-vs-preserve family for trailing comments in template expressions
