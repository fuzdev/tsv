# pre_block_head_sequence_line_comment_prettier_divergence

A `//` in a **sequence expression's operand gap** inside a `{#if}` / `{#each}` head in a
whitespace-sensitive element (`<pre>`). The comment ends its own line, so the next operand
drops below it — inside the braced head, where no character is content, so the render is
unchanged.

tsv:

```svelte
<pre>{#if (a, // c
	b)}x{/if}</pre>
```

Prettier ejects the comment out of the element entirely, where `// c` becomes a text node that
renders on the page:

```svelte
<pre>{#if (a, b)}x{/if}</pre> // c
```

That output is not a fixed point either — its next pass moves the comment again, onto its own
line — so `audit_signature.txt` pins the whole chain. With **two** comments the ejection also
welds them: `// c // c2` on one line makes `// c2` part of `// c`'s text, so the second comment
stops being a comment at all.

The comment is authored in the gap between an operand and its `,`, and tsv trails it past that
comma — the pure-separator carve-out, since the comma is structure and the per-operand break
keeps a run of comments distinct. `unformatted_ours_pre_comma.svelte` is that authoring;
prettier's first pass on it lands on `output_prettier.svelte`, which the same signature pins.

## Reason

Relocating a comment out of the construct it was written in is content loss, not a layout
choice. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and the catalog entry in
[conformance_prettier_svelte.md §Svelte: Blocks](../../../../../docs/conformance_prettier_svelte.md#svelte-blocks).

## Related

- [pre_block_head_line_comment](../pre_block_head_line_comment_prettier_divergence/) — the delimiter gaps (`(`, `[`, `${`), where prettier's output stays inside the element
- [pre_block_head_bracket_close_line_comment](../pre_block_head_bracket_close_line_comment_prettier_divergence/) — the index→`]` gap, the other place prettier ejects out of the element
- [pre_block_head_long](../pre_block_head_long/) — the width rule this comment rule is not
