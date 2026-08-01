# svelte_options_bare_comment_prettier_divergence

A `<svelte:options>` carrying **only** a comment — no attributes at all. tsv keeps the
comment; prettier deletes it.

tsv:

```svelte
<svelte:options /* c */ />
```

Prettier:

```svelte
<svelte:options />
```

This is the empty-attribute-list arm of the `<svelte:options>` printer, a separate path
from the one
[svelte_options_attr_comment](../svelte_options_attr_comment_prettier_divergence/)
exercises: with no attributes the tag is written out directly, so the attribute-list
region is never consulted at all.

## Reason

A comment is content; deleting it is content loss. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and the catalog entry in
[§Svelte: Attributes](../../../../../docs/conformance_prettier.md#svelte-attributes).

## Related

- [svelte_options_attr_comment](../svelte_options_attr_comment_prettier_divergence/) — the same deletion with attributes present
- [svelte_options_attr_comment_line](../svelte_options_attr_comment_line_prettier_divergence/) — the `//` shapes
- [svelte_options_bare_comment_line](../svelte_options_bare_comment_line_prettier_divergence/) — the no-attribute comments that force the head to break
- [svelte_options](../svelte_options/) — the plain tag (no comment)
