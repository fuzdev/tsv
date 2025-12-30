<div>
	<!-- Long arrow body: prettier doesn't wrap, we do -->
	{#each a.filter((item) => item.a && item.b && item.c && item.d && item.e && item.f && item.g && item.h && item.i) as item}{item}{/each}

	<!-- Long function args: prettier doesn't wrap, we do (101 chars) -->
	{#each fn00000(aaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbb, ccccccccccccccccccc) as item}{item}{/each}
</div>
