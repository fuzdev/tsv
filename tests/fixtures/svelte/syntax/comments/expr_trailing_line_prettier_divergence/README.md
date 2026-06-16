# expr_trailing_line_prettier_divergence

The line-comment counterpart of [expr_trailing](./../expr_trailing_prettier_divergence/):
prettier drops trailing comments in template expressions; tsv preserves them. Because a
`//` line comment runs to end of line, the closing `}` cannot follow it on the same line —
it would be swallowed on reparse — so tsv keeps the comment trailing the expression and
moves `}` to its own line.

tsv:

```svelte
{foo // c
}
```

Prettier: `{foo}` (comment stripped).

The closing brace stays in JS/expression context here, so the comment cannot be deferred
past `}` (the way a trailing line comment is in a TypeScript statement, via `lineSuffix`):
text after `}` is Svelte template **text**, so `{foo} // c` would render the literal
`// c` on the page. Keeping `}` on the next line is the only placement that both preserves
the comment and stays idempotent.

Affected contexts mirror the block-comment sibling, plus the buffer-printed tags:
`{expr}`, `{@html}`, `{@render}`, `{@debug}`, `{@const}`, `{#if}` / `{:else if}`,
`{#each}` (collection and key), `{#await}`, `{#key}`, `{...spread}`, `bind:value={}`,
`{@attach}`, `data-attr={}`, `on:event={}`, `class:name={}`, `use:action={}`.

`{@const x = value // c}` drops the comment like the rest. Under
prettier-plugin-svelte 3.5.2 this one case instead produced broken output with an
unmatched paren (`{@const y = it) …} // c`); 4.x drops the comment cleanly, so it is
once again a usable oracle.

## Reason

User comments are valuable and shouldn't be silently removed. The comments are
syntactically valid in these positions. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy).

## Related

- [expr_trailing](./../expr_trailing_prettier_divergence/) — same divergence for block comments (inline, single-line)
- [debug_comment](../../tags/debug/debug_comment_prettier_divergence/) — same pattern for `{@debug}`
