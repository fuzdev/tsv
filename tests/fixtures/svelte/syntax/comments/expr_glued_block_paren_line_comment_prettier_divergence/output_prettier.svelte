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
	{b ??
		/* c6 */ // d6
		a}
</p>
<!-- the same cells with the `//` glued to the `(`, where it keeps its line: the
	opening-delimiter rule -->
<p>
	{// h1
	a}
</p>
<p
	title={// h2
	a}
></p>
{#if // h3
a}
	text
{/if}
{@html // h4
a}
<p>
	{b ?? // h6
		/* g6 */ a}
</p>
