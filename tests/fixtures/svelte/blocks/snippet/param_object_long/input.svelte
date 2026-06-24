<!-- single object-pattern param, head is 100 chars - stays hugged inline -->
{#snippet fn({ aaaaaaaaaa, bbbbbbbbbb, cccccccccc, dddddddddd, eeeeeeeeee, ffffffffff, ggggggggg })}
	<div>text</div>
{/snippet}

<!-- single object-pattern param, head is 101 chars - hugs and expands one prop per line -->
{#snippet fn({
	aaaaaaaaaa,
	bbbbbbbbbb,
	cccccccccc,
	dddddddddd,
	eeeeeeeeee,
	ffffffffff,
	gggggggggg
})}
	<div>text</div>
{/snippet}
