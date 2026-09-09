# pre_block_head_line_comment_prettier_divergence

A `//` inside a `{#if}` / `{#each}` head in a whitespace-sensitive element (`<pre>`), at all
three head sites the whitespace-sensitive builder owns — the `{#if}` test, the `{#each}`
expression, and the `{#each}` key.

Such a head never wraps on **width**: a line break added there is rendered content
([pre_block_head_long](../pre_block_head_long/) pins that, and the inert 100/101 boundary). A
`//` is not a width question — it runs to end of line and so cannot share one — and the break
it forces lands inside the braced head, where no character is content, so the render is
unchanged. tsv opens the head around the comment and nothing else.

tsv:

```svelte
<pre>{#if typeof ( // c
		ast) === 'object'}a{/if}</pre>
```

Prettier's answer splits by delimiter. At the **comment-holder `(`** it keeps the comment on
that line too and differs only in the space after the `(`:

```svelte
<pre>{#if typeof (// c
		ast) === 'object'}a{/if}</pre>
```

At a **computed member's `[`** it relocates the comment ahead of the bracket, re-binding it to
the object rather than the index:

```svelte
<pre>{#if obj // c
	[key]}a{/if}</pre>
```

At a **template interpolation's `${`** both formatters agree, which is why that case is here
as the control.

The last two cases are the **own-line** authoring at the first two delimiters. The break
*above* the comment is as much an obligation as the one below a glued one: collapsed onto the
delimiter the comment becomes glued, which the next pass re-spaces — a two-pass document, and
a relocation rather than a layout choice, since own-line-ness is a source question. Prettier
relocates them: onto the `(`'s line at the comment holder, and ahead of the whole member at
the bracket.

## Reason

A same-line comment trails the token before it; relocating it re-binds it to another token.
The space after the opening delimiter is tsv's uniform answer at every delimiter it keeps a
comment on. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy),
the catalog entry in
[conformance_prettier_svelte.md §Svelte: Blocks](../../../../../docs/conformance_prettier_svelte.md#svelte-blocks),
and the opening-delimiter rule in
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).

## Related

- [pre_block_head_bracket_close_line_comment](../pre_block_head_bracket_close_line_comment_prettier_divergence/) — the index→`]` gap, where prettier ejects the comment out of the element instead
- [pre_block_head_long](../pre_block_head_long/) — the width rule this comment rule is not
- [ws_sensitive_attr_comment_line](../ws_sensitive_attr_comment_line_prettier_divergence/) — the same licence spent at the head's `>` instead of inside the braces
