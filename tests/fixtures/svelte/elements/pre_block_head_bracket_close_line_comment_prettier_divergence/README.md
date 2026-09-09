# pre_block_head_bracket_close_line_comment_prettier_divergence

A `//` in a computed member's index→`]` gap inside a `{#if}` / `{#each}` head in a
whitespace-sensitive element (`<pre>`). The comment ends its own line, so `]` drops below it —
inside the braced head, where no character is content, so the render is unchanged.

tsv:

```svelte
<pre>{#if obj[key // c
	]}a{/if}</pre>
```

Prettier ejects the comment out of the element entirely, where `// c` becomes a text node that
renders on the page:

```svelte
<pre>{#if obj[key]}a{/if}</pre> // c
```

That output is not a fixed point either — its next pass moves the comment again, onto its own
line — so `audit_signature.txt` pins the whole chain.

## Reason

Relocating a comment out of the construct it was written in is content loss, not a layout
choice. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and the catalog entry in
[conformance_prettier_svelte.md §Svelte: Blocks](../../../../../docs/conformance_prettier_svelte.md#svelte-blocks).

## Related

- [pre_block_head_line_comment](../pre_block_head_line_comment_prettier_divergence/) — the bracket's other end (`[`→index) and the other delimiters, where prettier's output stays well-formed
- [ws_sensitive_attr_comment_line](../ws_sensitive_attr_comment_line_prettier_divergence/) — the same ejection at the whitespace-sensitive head's `>`
