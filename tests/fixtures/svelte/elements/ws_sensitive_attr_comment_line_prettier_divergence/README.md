# ws_sensitive_attr_comment_line_prettier_divergence

A `//` line comment in the attribute list of a whitespace-sensitive element — `<pre>`, and
`<textarea>` — where the `>` normally **hugs** the last attribute so that no character is added
to the literal content. A `//` runs to end of line, so the `>` cannot share that line: tsv breaks
before it. The break lands inside the tag, where no character is content, so the rendered text is
untouched.

tsv:

```svelte
<pre data-attr="value" // c
	>text</pre
>

<pre // c
></pre>
```

With content the `>` takes the next line one level in and the closing tag dangles — the layout an
over-width `<textarea>` already takes, pinned by
[elements/textarea_attrs_long](../textarea_attrs_long/). With no content there is nothing for the
`>` to protect, so it sits at base indent, the shape every other element takes
([attributes/comment_trailing_same_line](../../attributes/comment_trailing_same_line/)).
An **own-line** `//` before the `>` carries its line break with it — the attributes wrap one per
line ([ws_sensitive_attr_comment_own_line](../ws_sensitive_attr_comment_own_line/)'s rule) and the
`>` keeps the next line, one level in.

Prettier **ejects the comment out of the element**:

```svelte
<pre
	data-attr="value">text</pre> // c
```

`// c` is now a text node in the template — it renders on the page — and prettier's own next pass
moves it again (`</pre>⏎// c`), so its first output is not a fixed point.
`<textarea>` takes the same corruption one place further in, into the closing tag
(`</textarea // c⏎>`), which the next pass reads as junk and drops, deleting the comment. With no
attribute in the list prettier deletes the comment outright at once — the empty-list rule of
[attributes/comment_no_attributes](../../attributes/comment_no_attributes_prettier_divergence/).

The hug is why this position is its own case: everywhere else the `>` already has its own line, so
a trailing `//` needs no special handling.

## Reason

A comment is content, and moving one out of the element that held it changes what the page
renders. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and the catalog entry in
[§Svelte: Attributes](../../../../../docs/conformance_prettier_svelte.md#svelte-attributes).

## Related

- [elements/textarea_attrs_long](../textarea_attrs_long/) — the same broken-`>` layout, forced by width
- [elements/pre_closing_tag](../pre_closing_tag/) — the dangling close tag in `<pre>` content
- [elements/ws_sensitive_self_closing_kinds](../ws_sensitive_self_closing_kinds_prettier_divergence/) — the other question this printing path answers on its own
- [attributes/comment_no_attributes](../../attributes/comment_no_attributes_prettier_divergence/) — the empty-list deletion, here as the no-attribute arm
- [attributes/comment_trailing_same_line](../../attributes/comment_trailing_same_line/) — a trailing `//` where the `>` already has its own line (matches prettier)
