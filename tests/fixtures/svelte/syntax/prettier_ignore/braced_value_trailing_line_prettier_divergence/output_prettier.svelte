<!-- a frozen value's trailing line comment already ends the content with its own break, so
	the closing `}` reuses it — no second break, and no blank line above it -->
<p>
	{// prettier-ignore
	aaa  +  bbb}
</p>

<div
	prop={// prettier-ignore
	ccc  +  ddd}
></div>

<input
	bind:value={
		// prettier-ignore
		eee  .  fff
	}
/>

<!-- the prefixed heads already drop the second break, so the parity is the assertion -->
{@html // prettier-ignore
ggg  +  hhh}
