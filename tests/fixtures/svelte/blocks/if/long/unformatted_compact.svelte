<div>
	<!-- 2-chain: 100 chars - stays inline (at boundary) -->
	{#if a.filter((item)=>item.x).some((item)=>item.yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy)}{x}{/if}

	<!-- 2-chain: 101 chars - wraps (exceeds print_width) -->
	{#if a.filter((item)=>item.x).some((item)=>item.yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy)}{x}{/if}

	<!-- 3+ chain: always wraps regardless of width -->
	{#if a.filter((item)=>item.x).map((item)=>item.y).some((item)=>item.z)}{x}{/if}
</div>
