<div>
	<!-- Long arrow body: prettier doesn't wrap, we do -->
	{#key a.filter(
		(item) =>
			item.a && item.b && item.c && item.d && item.e && item.f && item.g && item.h && item.i,
	)}{x}{/key}

	<!-- Long function args: prettier doesn't wrap, we do (101 chars) -->
	{#key fn00000(
		aaaaaaaaaaaaaaaaaaa,
		bbbbbbbbbbbbbbbbbbb,
		ccccccccccccccccccc,
		ddddddddddd,
	)}{x}{/key}
</div>
