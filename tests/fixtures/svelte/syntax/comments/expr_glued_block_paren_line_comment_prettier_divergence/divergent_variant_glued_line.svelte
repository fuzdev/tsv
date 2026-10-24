<!-- a block comment glued to a grouping `(` binds to the parenthesized value, so the pair is
	kept when its interior opens with a `//` -->
<p>
	{// d1
	a}
</p>
<p
	title={// d2
	a}
></p>
{#if // d3
a}
	text
{/if}
{@html // d4
a}
<p>
	{b ?? // d6
		/* c6 */ a}
</p>
