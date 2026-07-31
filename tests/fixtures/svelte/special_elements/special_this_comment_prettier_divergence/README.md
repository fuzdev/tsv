# special_this_comment_prettier_divergence

`<svelte:element>` / `<svelte:component>` carry their `this` binding in the AST's element
kind rather than in `attributes`, so the printer synthesizes `this={…}` at the head of the
attribute list. A comment in that region keeps the **side of `this`** the author wrote it
on; prettier either deletes it (when `this` is the only attribute) or moves it after
`this` onto its own line.

tsv:

```svelte
<svelte:element // c
	this={x}
	data-attr="value"
/>

<svelte:element
	this={x} // c
	data-attr="value"
/>
```

Prettier collapses both authorings to one form, losing which side of `this` the comment
was on:

```svelte
<svelte:element
	this={x}
	// c
	data-attr="value"
/>
```

and with no other attribute it deletes the comment outright (`<svelte:element this={x} />`).

The two authorings are two fixed points, exactly as for an ordinary element's tag-name
and between-attribute positions — see
[comment_same_line](../../attributes/comment_same_line_prettier_divergence/). Because
`this` is synthesized rather than parsed as an attribute, the region before it has no
attribute to anchor against; keeping the comment on its authored side is what makes the
layout a fixed point at all.

## Reason

Comment placement is a deliberate authoring choice and tsv preserves it; deleting a
comment is content loss. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and the catalog entry in
[§Svelte: Attributes](../../../../../docs/conformance_prettier.md#svelte-attributes).

## Related

- [comment_no_attributes](../../attributes/comment_no_attributes_prettier_divergence/) — the same deletion in tags with no attributes at all
- [special_this_duplicate](../special_this_duplicate/) — the `this` binding's parse-side rules
- [svelte_element_this_string_attr_comment](../svelte_element_this_string_attr_comment/) — a comment *inside* an attribute value on the same tag (matches prettier)
