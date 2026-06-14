# expr_trailing_prettier_divergence

Prettier drops trailing comments in template expressions. tsv preserves them.

tsv: `{#if condition /* c */}` (preserved)
Prettier: `{#if condition}` (comment stripped)

Affected contexts: `{expr}`, `{@html}`, `{@render}`, `{@const}`, `{#if}`, `{:else if}`, `{#each}` (collection and key), `{#await}`, `{#key}`, `{...spread}`, `bind:value={}`, `{@attach}`, `data-attr={}`, `on:event={}`, `class:name={}`, `use:action={}`, `style:prop={}`, `transition:fn={}`, `animate:fn={}`, `let:name={}`.

Prettier also produces broken output for `{@const x = value /* c */}` (unmatched paren in output).

## Reason

User comments are valuable and shouldn't be silently removed. The comments are syntactically valid in these positions.

## Related

- [debug_comment](../../tags/debug/debug_comment_prettier_divergence/) — same pattern for `{@debug}`
