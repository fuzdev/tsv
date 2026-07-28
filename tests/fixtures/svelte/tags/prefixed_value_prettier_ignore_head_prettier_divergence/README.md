# prefixed_value_prettier_ignore_head_prettier_divergence

An own-line directive in a **prefixed braced tag's** head gap — the `{@html`→value,
`{@render`→value and `{@attach`→value gaps — freezes that whole value. The prefix keyword
and the closing `}` are parent-owned and stay outside the slice:

```svelte
{@html
	// prettier-ignore
	aaa  +  bbb
}
```

Prettier **relocates** the directive flush onto the prefix's line (`{@html // prettier-ignore`)
and freezes anyway. tsv keeps the line the author gave it and breaks the head into the block
form its unprefixed sibling already uses (`{⏎…⏎}` — see
[bind/value_prettier_ignore_head](../../directives/bind/value_prettier_ignore_head/)). That is
load-bearing rather than cosmetic: a head-trailing directive is inert under the placement floor,
so following the relocation would lose the freeze on tsv's own second pass.

**Both spellings** behave alike — placement keys the freeze, not the comment's spelling. A
sibling tag the freeze does not reach still normalizes.

## Reason

Rule A binds a directive to the node that follows it; ◆comment_preservation. See
[conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
