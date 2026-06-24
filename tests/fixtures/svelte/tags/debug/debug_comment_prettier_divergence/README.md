# debug_comment_prettier_divergence

Prettier strips comments from `{@debug}` expressions. tsv preserves them.

tsv: `{@debug /* comment */ a}` (preserved)
Prettier: `{@debug a}` (comment stripped)

## Reason

Content preservation. Comments in debug statements often carry important
context (expected values, why the variable is being debugged); stripping them
is silent content loss of developer intent.

See [conformance_prettier.md §Svelte: Elements](../../../../../../docs/conformance_prettier.md#svelte-elements)
(the `@debug comments` catalog entry).

## Related

- [expr_trailing](../../../syntax/comments/expr_trailing_prettier_divergence/) — same pattern across all template expressions
