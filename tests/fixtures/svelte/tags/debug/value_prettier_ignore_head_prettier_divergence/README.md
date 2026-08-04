# value_prettier_ignore_head_prettier_divergence

An own-line directive in the `{@debug`→identifiers gap freezes the identifier **list** — the
run from the first identifier to the last, with the prefix and the closing `}` parent-owned:

```svelte
{@debug
	// prettier-ignore
	aaa ,  bbb
}
```

Prettier has no freeze to compare against here: it **deletes the comment** along with every
other comment inside a `{@debug}` (`{@debug aaa, bbb}`) — ◆prettier_bug, the same content loss
[debug_comment](../debug_comment_prettier_divergence/) already catalogs for ordinary comments.
tsv preserves the comment, so it also has to answer where the comment goes and what it freezes;
it answers both the way every other prefixed head does — own-line, freezing what follows.

A sibling tag the freeze does not reach still normalizes.

## Reason

Rule A binds a directive to the node that follows it; ◆comment_preservation ◆prettier_bug. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
