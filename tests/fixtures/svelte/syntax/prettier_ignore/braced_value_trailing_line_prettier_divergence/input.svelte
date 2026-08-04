<!-- a frozen value's trailing line comment already ends the content with its own break, so
	the closing `}` reuses it — no second break, and no blank line above it -->
<p>
	{
		// prettier-ignore
		aaa  +  bbb // c
	}
</p>

<div
	prop={
		// prettier-ignore
		ccc  +  ddd // c
	}
></div>

<input
	bind:value={
		// prettier-ignore
		eee  .  fff // c
	}
/>

<!-- the prefixed heads already drop the second break, so the parity is the assertion —
	`{@debug}` included, whose head freezes an identifier list and emits its own trailing run -->
{@html
	// prettier-ignore
	ggg  +  hhh // c
}
{@debug
	// prettier-ignore
	iii ,  jjj // c
}
