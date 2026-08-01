# svelte_options_attr_comment_line_prettier_divergence

The `//` counterpart of
[svelte_options_attr_comment](../svelte_options_attr_comment_prettier_divergence/): a line
comment in the `<svelte:options>` attribute list keeps its authored line and forces the
head to break; prettier deletes both comments and collapses the tag.

tsv:

```svelte
<svelte:options // c1
	runes // c2
/>
```

Prettier:

```svelte
<svelte:options runes />
```

The broken shape is the one `<svelte:options>` already takes when its attributes exceed
the print width — attributes indented one level, `/>` on its own line at base — see
[svelte_options_long](../svelte_options_long/). A `//` runs to end of line, so the break
is forced whatever the width.

## Reason

A comment is content; deleting it is content loss. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and the catalog entry in
[§Svelte: Attributes](../../../../../docs/conformance_prettier.md#svelte-attributes).

## Related

- [svelte_options_attr_comment](../svelte_options_attr_comment_prettier_divergence/) — block comments at all three positions
- [svelte_options_bare_comment](../svelte_options_bare_comment_prettier_divergence/) — a `<svelte:options>` with no attributes at all
- [svelte_options_bare_comment_line](../svelte_options_bare_comment_line_prettier_divergence/) — the same `//` / own-line shapes with no attributes
- [svelte_options_long](../svelte_options_long/) — the same broken shape, forced by width
