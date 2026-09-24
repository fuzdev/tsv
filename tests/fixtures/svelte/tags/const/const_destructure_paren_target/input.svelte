<!-- a {@const} id is read as an assignment target, so a parenthesized target is grouping -->
{#if a}
	{@const [b1, c1 = 1] = expr}
	{@const { b2: c2, ...d2 } = expr}
	{@const [...b3] = expr}
	{@const {
		b4: [c4.d]
	} = expr}
	{c1}
{/if}
