# title_head_comment_prettier_divergence

A comment in the head of a `<title>` that is a child of `<svelte:head>`. Two claims in one
fixture: the comment survives (prettier deletes it), and the head lays out like the rest of
the whitespace-preserving family.

tsv:

```svelte
<svelte:head>
	<title /* c1 */>text1</title>

	<title
		/* c2 */>text2</title
	>

	<title // c3
		>text3</title
	>
</svelte:head>
```

Prettier drops every comment outright:

```svelte
<svelte:head>
	<title>text1</title>

	<title>text2</title>

	<title>text3</title>
</svelte:head>
```

## Cases

- **inline `/* c1 */`** — the list fits flat, so the `>` hugs the comment and nothing dangles.
- **own-line `/* c2 */`** — the comment keeps its own line, which wraps the list; the `>` hugs
  the last member and the closing tag dangles.
- **`// c3`** — a `//` runs to end of line, so the `>` cannot share it and takes the next line
  one level in; the comment itself stays on the tag-name line the author wrote it on.

## Reason

**◆content_preservation** for the comment: an attribute-less head has nothing for prettier to
anchor a comment to, so it deletes it — the empty-list rule of
[attributes/comment_no_attributes](../../attributes/comment_no_attributes_prettier_divergence/),
which pins the same deletion for `<div>`, components, void elements, `<slot>`, the special
elements that take no `this`, and the whitespace-sensitive `<pre>` / `<textarea>`. A comment is
content; deleting it is content loss.

The **layout** is the whitespace-preserving family's, not a choice this kind makes for itself. A
head `<title>` prints its content verbatim
([title_content_verbatim](../title_content_verbatim_prettier_divergence/)), which puts it in the
`<pre>` / `<textarea>` class, and there the `>` hugs the last list member so no character is
added to the literal content — [elements/ws_sensitive_attr_comment_own_line](../../elements/ws_sensitive_attr_comment_own_line/)
for the own-line comment, [elements/ws_sensitive_attr_comment_line](../../elements/ws_sensitive_attr_comment_line_prettier_divergence/)
for the `//`. `<title>` is inline by the same classification that gives `<textarea>` its dangling
closing `>` and `<pre>` an intact one, so it dangles.

Attributes on a `<title>` are a Svelte **compile** error (`title_illegal_attribute`), so a
comment is the only list member its head can hold in a component that compiles — which is why
this position needs a fixture of its own rather than riding the attribute cases. The formatter
still prints what it is handed, and an attribute-bearing head takes the same shape.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and the catalog entry in
[conformance_prettier_svelte.md §Svelte: Attributes](../../../../../docs/conformance_prettier_svelte.md#svelte-attributes).

## Related

- [title_content_verbatim](../title_content_verbatim_prettier_divergence/) — the content half of the same kind
- [elements/ws_sensitive_head_attrs_wrap](../../elements/ws_sensitive_head_attrs_wrap/) — the family's head shape with real attributes (matches prettier)
- [attributes/comment_no_attributes](../../attributes/comment_no_attributes_prettier_divergence/) — the empty-list deletion across every other tag kind
- [attributes/comment_with_attribute](../../attributes/comment_with_attribute/) — the discriminator: one real attribute and both formatters keep the comment
