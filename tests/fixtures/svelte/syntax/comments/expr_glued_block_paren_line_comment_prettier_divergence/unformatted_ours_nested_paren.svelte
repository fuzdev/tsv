<!-- a block comment glued to a grouping `(` binds to the parenthesized value, so the pair is
	kept when its interior opens with a `//` -->
<p>
	{/* c1 */ ((
		// d1
		a
	))}
</p>
<p
	title={/* c2 */ ((
		// d2
		a
	))}
></p>
{#if /* c3 */ ((
	// d3
	a
))
}
	text
{/if}
{@html /* c4 */ ((
	// d4
	a
))}
<p>
	{b ??
		/* c6 */ ((
			// d6
			a
		))}
</p>
<!-- the same cells with the `//` glued to the `(`, where it keeps its line: the
	opening-delimiter rule -->
<p>
	{/* g1 */ ( // h1
		a
	)}
</p>
<p
	title={/* g2 */ ( // h2
		a
	)}
></p>
{#if /* g3 */ ( // h3
	a
)
}
	text
{/if}
{@html /* g4 */ ( // h4
	a
)}
<p>
	{b ??
		/* g6 */ ( // h6
			a
		)}
</p>
