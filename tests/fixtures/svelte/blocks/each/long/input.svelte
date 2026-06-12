<div>
	<!-- 2-chain: 100 chars - stays inline (at boundary) -->
	{#each a.filter((item) => item.x).filter((item) => item.yyyyyyyyyyyyyyyyyyy) as item}{item}{/each}

	<!-- 2-chain: 101 chars - wraps (exceeds print_width) -->
	{#each a
		.filter((item) => item.x)
		.filter((item) => item.yyyyyyyyyyyyyyyyyyyy) as item}{item}{/each}

	<!-- 3+ chain: always wraps regardless of width -->
	{#each a
		.filter((item) => item.x)
		.map((item) => item.y)
		.filter((item) => item.z) as item}{item}{/each}
</div>
