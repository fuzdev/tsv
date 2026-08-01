# special_this_mid_list_comment_prettier_divergence

A `this` binding written **after** other attributes is hoisted to the head of the list by
both formatters — but a comment must stay with the token it binds when the binding moves.
tsv routes each comment by its binding: a same-line comment trails the token before it, an
own-line comment leads the token after it (the model
[comment_same_line](../../attributes/comment_same_line_prettier_divergence/) and
[comment](../../attributes/comment/) pin for ordinary attribute lists).

Only two positions bind the head itself and so keep it while `this` hoists in underneath:
an **inline comment in the tag-name gap** (it trails the tag name, the one token that
never moves) and an **own-line comment immediately leading `this`** (it travels with the
binding). Those are this fixture's cases.

tsv:

```svelte
<svelte:element /* c1 */ this={x} data-attr="value" />

<svelte:element
	/* c3 */
	this={x}
	data-attr="value"
/>
```

Prettier re-glues the tag-name-gap comment to the attribute that followed it, and moves
the own-line comment leading `this` to the **end** of the list, after attributes it never
referred to:

```svelte
<svelte:element this={x} /* c1 */ data-attr="value" />

<svelte:element
	this={x}
	data-attr="value"
	/* c3 */
/>
```

A comment bound to one of the *other* attributes is not moved at all — it keeps its
authored location while `this` hoists past, and both formatters agree, so those shapes are
the plain [special_this_mid_list_comment](../special_this_mid_list_comment/) fixture.

## Reason

Comment placement is a deliberate authoring choice and tsv preserves the comment's
binding when the attribute list is reordered around it. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and the catalog entry in
[§Svelte: Attributes](../../../../../docs/conformance_prettier.md#svelte-attributes).

## Related

- [special_this_mid_list_comment](../special_this_mid_list_comment/) — comments bound to the other attributes, which stay put (matches prettier)
- [special_this_comment](../special_this_comment_prettier_divergence/) — comments around a source-first `this`
- [comment_same_line](../../attributes/comment_same_line_prettier_divergence/) — the trailing rule in ordinary attribute lists
