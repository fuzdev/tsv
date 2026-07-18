# debug_multiline_comment_prettier_divergence

Prettier strips comments from `{@debug}` expressions. tsv preserves them. The
multi-line sibling of [debug_comment](../debug_comment_prettier_divergence/): a
`*`-alignable multi-line block comment is reindented to context (matching every other
comment position — Prettier's `printIndentableBlockComment` behavior), not left verbatim.

tsv:

```svelte
{@debug /* c
 */ x}
```

Prettier: `{@debug x}` (comment stripped).

## Reason

Content preservation. Comments in debug statements often carry important context;
stripping them is silent content loss of developer intent. A multi-line block comment
reindents to context like it does everywhere else.

See [conformance_prettier.md §Svelte: Elements](../../../../../../docs/conformance_prettier.md#svelte-elements)
(the `@debug comments` catalog entry).

## Related

- [debug_comment](../debug_comment_prettier_divergence/) — the single-line case
