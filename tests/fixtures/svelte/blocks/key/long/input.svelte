<div>
	<!-- 2-chain: 100 chars - stays inline (at boundary) -->
	{#key a.filter((item) => item.x).some((item) => item.yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy)}{x}{/key}

	<!-- 2-chain: 101 chars - wraps (exceeds print_width) -->
	{#key a
		.filter((item) => item.x)
		.some((item) => item.yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy)}{x}{/key}

	<!-- 3+ chain: always wraps regardless of width -->
	{#key a
		.filter((item) => item.x)
		.map((item) => item.y)
		.some((item) => item.z)}{x}{/key}
</div>
