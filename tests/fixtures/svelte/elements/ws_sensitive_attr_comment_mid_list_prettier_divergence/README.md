# ws_sensitive_attr_comment_mid_list_prettier_divergence

A same-line `//` between two attributes of a whitespace-sensitive element — `<textarea>` with
content, and an inline element inside `<pre>`. The comment forces the attribute after it onto a
fresh line, so the list wraps
([ws_sensitive_attr_comment_own_line](../ws_sensitive_attr_comment_own_line/)'s rule), and the
comment itself stays trailing the token it was written after — the same
[comment_same_line](../../attributes/comment_same_line_prettier_divergence/) rule every other
attribute list applies. The `>` still hugs the last attribute, so no character is added to the
literal content.

tsv:

```svelte
<textarea
	data-attr1="value1" // c
	data-attr2="value2">text</textarea
>
```

Prettier relocates the comment onto its own line:

```svelte
<textarea
	data-attr1="value1"
	// c
	data-attr2="value2">text</textarea
>
```

The layout is otherwise identical — this host pins only the comment's position. It is a
separate fixture from
[ws_sensitive_attr_comment_line](../ws_sensitive_attr_comment_line_prettier_divergence/) because
prettier's behavior differs in kind: mid-list its output is well-formed relocation, while
trailing (adjacent to the `>`) it ejects the comment out of the element entirely.

## Reason

A same-line comment trails the token before it; relocating it re-binds it to the token after.
See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and the catalog entry in
[§Svelte: Attributes](../../../../../docs/conformance_prettier.md#svelte-attributes).

## Related

- [attributes/comment_same_line](../../attributes/comment_same_line_prettier_divergence/) — the same rule at an ordinary element head
- [ws_sensitive_attr_comment_own_line](../ws_sensitive_attr_comment_own_line/) — the wrap-and-hug layout this host takes, pinned where both formatters agree
- [ws_sensitive_attr_comment_line](../ws_sensitive_attr_comment_line_prettier_divergence/) — the trailing `//`, where prettier's output corrupts instead
