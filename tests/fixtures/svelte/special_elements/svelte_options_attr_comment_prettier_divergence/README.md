# svelte_options_attr_comment_prettier_divergence

A comment in the `<svelte:options>` attribute list is kept where the author wrote it;
prettier deletes it — at every position, whether or not real attributes surround it.

tsv:

```svelte
<svelte:options /* c1 */ runes /* c2 */ namespace="svg" /* c3 */ />
```

Prettier:

```svelte
<svelte:options runes namespace="svg" />
```

`<svelte:options>` is hoisted out of the fragment and printed at its canonical position
from its own builder, so — unlike every other tag — the deletion does not depend on the
attribute list being empty: it happens with attributes present, at the leading,
between-attribute, and trailing positions alike. Only one `<svelte:options>` may appear
per component, so each shape needs its own fixture.

## Reason

A comment is content; deleting it is content loss. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and the catalog entry in
[§Svelte: Attributes](../../../../../docs/conformance_prettier.md#svelte-attributes).

## Related

- [svelte_options_attr_comment_line](../svelte_options_attr_comment_line_prettier_divergence/) — the same list with `//` comments (which force the break)
- [svelte_options_bare_comment](../svelte_options_bare_comment_prettier_divergence/) — a `<svelte:options>` with no attributes at all
- [svelte_options_comment](../svelte_options_comment/) — an HTML comment *before* the tag (matches prettier)
