<div>
	<!-- fits inline (head + inline body <= 100): stays fully inline -->
	{#key fn00000(aaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbb, ccccccccccccccccccccccccccccccc)}{x}{/key}

	<!-- single call whose args wrap: ) dedents to base, then } dangles on its own line (divergence) -->
	{#key fn00000(aaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbb, ccccccccccccccccccc, dddddddddddddddddddd)}{x}{/key}

	<!-- member-call (.filter) with arrow body: call breaks open, ) dedents, } dangles (divergence) -->
	{#key a.filter((item) => item.a && item.b && item.c && item.d && item.e && item.f && item.g && item.h && item.i)}{x}{/key}

	<!-- binary chain expression: wraps, } dangles (divergence) -->
	{#key aaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbb && cccccccccccccccc && dddddddddddddddd && eeeeeeeeeeeeeeee && ffff}{x}{/key}

	<!-- 2-group member chain at the boundary: fits fully inline -->
	{#key a.filter((item) => item.x).filter((item) => item.yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy)}{x}{/key}

	<!-- 2-group member chain, head alone > 100: wraps, } dangles, body expands (divergence) -->
	{#key a
		.filter((item) => item.x)
		.filter((item) => item.yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy)}{x}{/key}

	<!-- 3+ group member chain: always wraps regardless of width (divergence) -->
	{#key a
		.filter((item) => item.x)
		.map((item) => item.y)
		.filter((item) => item.z)}{x}{/key}
</div>
