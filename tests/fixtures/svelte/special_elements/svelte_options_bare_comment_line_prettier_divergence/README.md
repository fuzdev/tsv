# svelte_options_bare_comment_line_prettier_divergence

The `//` and own-line counterpart of
[svelte_options_bare_comment](../svelte_options_bare_comment_prettier_divergence/): a
`<svelte:options>` with no attributes whose comments cannot stay inline — a `//` on the
tag-name line and an own-line block comment. Each keeps its authored line and the head
breaks; prettier deletes both and collapses the tag.

tsv:

```svelte
<svelte:options // c1
	/* c2 */
/>
```

Prettier:

```svelte
<svelte:options />
```

Same broken shape as [svelte_options_attr_comment_line](../svelte_options_attr_comment_line_prettier_divergence/),
here with nothing but the comments in the attribute-list region: a `//` runs to end of
line, so the `/>` must drop to the next line whatever the width, and an own-line comment
keeps its own line.

## Reason

A comment is content; deleting it is content loss. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and the catalog entry in
[§Svelte: Attributes](../../../../../docs/conformance_prettier.md#svelte-attributes).

## Related

- [svelte_options_bare_comment](../svelte_options_bare_comment_prettier_divergence/) — the inline `/* */` form, which keeps the tag on one line
- [svelte_options_attr_comment_line](../svelte_options_attr_comment_line_prettier_divergence/) — the same `//` shapes with attributes present
- [svelte_options_attr_comment](../svelte_options_attr_comment_prettier_divergence/) — block comments at all three attribute-list positions
