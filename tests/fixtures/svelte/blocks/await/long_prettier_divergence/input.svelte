<div>
	<!-- fits inline (head + inline body <= 100): stays fully inline -->
	{#await getData(aaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbb, cccccccccccccccccccc) then r}{r}{/await}

	<!-- single call whose args wrap: ) dedents to base, then } dangles on its own line (divergence) -->
	{#await getData(
		aaaaaaaaaaaaaaaaaaa,
		bbbbbbbbbbbbbbbbbbb,
		ccccccccccccccccccc,
		ddddddddddddddddddd
	) then r}
		{r}
	{/await}

	<!-- member-call (.filter) with arrow body: call breaks open, ) dedents, } dangles (divergence) -->
	{#await a.filter(
		(item) => item.a && item.b && item.c && item.d && item.e && item.f && item.g && item.h && item.i
	) then item}
		{item}
	{/await}

	<!-- binary chain expression: wraps, } dangles (divergence) -->
	{#await aaaaaaaaaaaaaaaa &&
		bbbbbbbbbbbbbbbb &&
		cccccccccccccccc &&
		dddddddddddddddd &&
		eee
	then r}
		{r}
	{/await}

	<!-- 2-group member chain at the boundary: fits fully inline -->
	{#await a.filter((item) => item.x).filter((item) => item.yyyyyyyyyyyyyyyy) then r}{r}{/await}

	<!-- 2-group member chain, head alone > 100: wraps, clause + } drop to base, body expands (divergence) -->
	{#await a
		.filter((item) => item.x)
		.filter((item) => item.yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy)
	then r}
		{r}
	{/await}

	<!-- 3+ group member chain: always wraps regardless of width (divergence) -->
	{#await a
		.filter((item) => item.x)
		.map((item) => item.y)
		.filter((item) => item.z)
	then r}
		{r}
	{/await}

	<!-- full {:then}/{:catch} form, middle-zone (head fits alone, construct overflows): pending body, each {:then}/{:catch} section + keyword, and {/await} each drop to their own line (divergence) -->
	{#await getData(aaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbb)}
		loading
	{:then value}
		{value}
	{:catch error}
		{error}
	{/await}
</div>
