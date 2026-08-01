# comment_no_attributes_prettier_divergence

A comment in the attribute-list region of a tag that carries **no attributes at all**
is kept where the author wrote it; prettier deletes it.

tsv:

```svelte
<div // c
></div>

<div /* c */></div>

<Comp /* c */ />
```

Prettier drops the comment outright:

```svelte
<div></div>

<div></div>

<Comp />
```

The loss is total — the comment does not move, it disappears — and it is keyed on the
attribute list being **empty**: one real attribute is enough for both formatters to keep
the comment, at every position in the list. Every tag kind is affected the same way:
HTML elements (with or without content), components, void elements, `<slot>`, the
special elements that take no `this` binding (`<svelte:head>`, `<svelte:window>`), and
the whitespace-sensitive elements (`<pre>`, `<textarea>`), whose head is printed by a
builder of its own.

Both comment kinds are affected, and each keeps its authored placement: a `//` on the
tag-name line stays there (forcing the `>` / `/>` to the next line), an own-line comment
keeps its own line, an inline `/* */` stays inline, and consecutive comments keep their
order — including a mixed `/* c1 */ // c2` run, where the block stays inline and the
`//` still forces the break.

## Reason

A comment is content; deleting it is content loss. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and the catalog entry in
[§Svelte: Attributes](../../../../../docs/conformance_prettier.md#svelte-attributes).

## Related

- [comment_with_attribute](../comment_with_attribute/) — the discriminator: one real attribute and both formatters keep the comment (not a divergence)
- [comment_same_line](../comment_same_line_prettier_divergence/) — where prettier *moves* a same-line `//` instead of deleting it
- [comment](../comment/) — own-line attribute-list comments with attributes present (matches prettier)
- [special_this_comment](../../special_elements/special_this_comment_prettier_divergence/) — the same region around a synthesized `this` binding
- [ws_sensitive_attr_comment_line](../../elements/ws_sensitive_attr_comment_line_prettier_divergence/) — the `//` forms in a whitespace-sensitive head, where the `>` hug is at stake
