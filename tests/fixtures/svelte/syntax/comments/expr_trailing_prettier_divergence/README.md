# expr_trailing_prettier_divergence

Prettier drops trailing comments in template expressions. tsv preserves them.

tsv: `{#if condition /* c */}` (preserved)
Prettier: `{#if condition}` (comment stripped)

Affected contexts: `{expr}`, `{@html}`, `{@render}`, `{@const}`, `{#if}`, `{:else if}`, `{#each}` (collection and key), `{#await}`, `{#key}`, `{...spread}`, `bind:value={}`, `{@attach}`, `data-attr={}`, `on:event={}`, `class:name={}`, `use:action={}`, `style:prop={}`, `transition:fn={}`, `animate:fn={}`, `let:name={}`.

(`{@const x = value /* c */}` drops the comment like the rest. Under
prettier-plugin-svelte 3.5.2 this one case instead produced broken output with an
unmatched paren — `{@const x = value) /* c */}`; 4.x drops the comment cleanly.)

## Reason

User comments are valuable and shouldn't be silently removed. The comments are syntactically valid
in these positions. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).

## Related

- [debug_comment](../../tags/debug/debug_comment_prettier_divergence/) — same pattern for `{@debug}`
