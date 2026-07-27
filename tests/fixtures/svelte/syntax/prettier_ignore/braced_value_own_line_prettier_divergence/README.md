# braced_value_own_line_prettier_divergence

An honored directive keeps its own line in a Svelte `{…}` **value** gap, so the value takes
the broken block form:

```svelte
<div
	class:active={
		// prettier-ignore
		a  &&  b
	}
></div>
```

Prettier pulls the directive flush against the `{` (`class:active={// prettier-ignore`) and
freezes anyway — it decides a directive by comment *attachment*, so the line the author gave
it carries no weight. tsv decides by **placement**, and a directive sharing its line with the
`{` is inert under the placement floor, so the relocated form would lose the freeze on tsv's
own second pass. The same rule already applies at the declaration-header gaps
([declarator head](../../../../typescript/declarations/variable/declarations_prettier_ignore_head_prettier_divergence/),
[keyword-gap own line](../../../../typescript/syntax/comments/keyword_gap_prettier_ignore_own_line_prettier_divergence/));
this is its `{…}`-value instance, shared by every braced value gap — event and class /
style directives here, and an expression tag.

`bind:` already writes the broken block form for a value that must break, so it matches
prettier and needs no divergence — see the ordinary sibling
[bind/value_prettier_ignore_head](../../../directives/bind/value_prettier_ignore_head/).

See [conformance_prettier.md §Format-ignore directive](../../../../../../docs/conformance_prettier.md#format-ignore-directive).
