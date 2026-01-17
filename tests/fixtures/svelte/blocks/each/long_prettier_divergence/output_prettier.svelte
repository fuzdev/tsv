<div>
	<!-- 100 chars - stays inline (at boundary, both agree) -->
	{#each fn00000(aaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbb, cccccccccccccccccc) as item}{item}{/each}

	<!-- 101 chars - args wrap (divergence: Prettier keeps inline) -->
	{#each fn00000(aaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbb, ccccccccccccccccccc) as item}{item}{/each}

	<!-- Long filter with arrow body (divergence: call breaks open) -->
	{#each a.filter((item) => item.a && item.b && item.c && item.d && item.e && item.f && item.g && item.h && item.i) as item}{item}{/each}
</div>
