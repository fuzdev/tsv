<!-- An inlining logical chain in a tag: the array/object hugs its operator, and with an -->
<!-- earlier operator to place, the continuation indents one level past the tag's own line -->
{#each items as item}
	{@const a = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
		cccccccccccccccccccccccccccccccccccc ?? [1]}
	{@const b = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ||
		cccccccccccccccccccccccccccccccccccc || { x: 1 }}

	<!-- {const}/{let} take the same shape through the declaration printer -->
	{const c = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
		cccccccccccccccccccccccccccccccccccc ?? [1]}

	<!-- A chain that does NOT end in a self-expanding value breaks after `=` instead -->
	{@const d =
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ?? cccccccccccccccccccccccccccccccccccc ?? eee}

	<!-- 100 chars - stays inline -->
	{@const e = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ?? ccccccccccccccccccccccccccccccccc ?? [1]}

	<!-- 101 chars - the chain breaks -->
	{@const f = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ?? cccccccccccccccccccccccccccccccccc ?? [1]}
	{a}{b}{c}{d}{e}{f}
{/each}
