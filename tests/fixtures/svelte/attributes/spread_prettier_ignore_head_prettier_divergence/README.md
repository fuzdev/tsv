# spread_prettier_ignore_head_prettier_divergence

An own-line directive in a spread attribute's `{...`→value gap freezes the whole value. The
`...` prefix and the closing `}` are parent-owned and stay outside the slice:

```svelte
<div
	{...
		// prettier-ignore
		aaa  .  bbb
	}
></div>
```

Prettier **relocates** the directive flush onto the `{...` line (`{...// prettier-ignore`) and
freezes anyway. tsv keeps the line the author gave it and breaks the head into the block form
its unprefixed sibling already uses (`{⏎…⏎}` — see
[bind/value_prettier_ignore_head](../../directives/bind/value_prettier_ignore_head/)): a
head-trailing directive is inert under the placement floor, so following the relocation would
lose the freeze on tsv's own second pass.

A sibling spread the freeze does not reach still normalizes.

## Reason

Rule A binds a directive to the node that follows it; ◆comment_preservation. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
