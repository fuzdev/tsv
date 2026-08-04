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

## Both spellings

Placement keys the freeze, not the spelling, so an own-line **block** directive owes the
identical layout:

```svelte
<p>
	{
		/* prettier-ignore */
		a  +  b
	}
</p>
```

That half has to be pinned separately because nothing makes it fall out: a `//` directive
ends in a hardline of its own, so the block form arrives for free, while `/*…*/` is emitted
inline and the `{…}` group is softline-hung — the collapsed `{/* prettier-ignore */ a  +  b}`
is precisely the inert placement above. Prettier collapses it and stays frozen (attachment,
again); tsv breaks and stays frozen.

The block half covers every braced value shape — a directive value, an expression tag, a
`bind:` value, and a `bind:` function-binding sequence — because they reach three different
builders that share one block-wrapping seam. `bind:` needs no divergence for the **line**
spelling, where it already writes the broken form and matches prettier: see the ordinary
sibling [bind/value_prettier_ignore_head](../../../directives/bind/value_prettier_ignore_head/),
and [bind/value_sequence_prettier_ignore_head](../../../directives/bind/value_sequence_prettier_ignore_head_prettier_divergence/)
for the sequence's freeze scope.

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
