<div>
	<!-- fits inline (head + inline body <= 100): stays fully inline -->
	{#each fn00000(aaaaaaaaaaaaaaaaaaa,bbbbbbbbbbbbbbbbbbb,cccccccccccccccccc) as item}{item}{/each}

	<!-- single call whose args wrap: ) dedents to base, then } dangles on its own line (divergence) -->
	{#each fn00000(aaaaaaaaaaaaaaaaaaa,bbbbbbbbbbbbbbbbbbb,ccccccccccccccccccc,ddddddddddddddddddd) as item}{item}{/each}

	<!-- member-call (.filter) with arrow body: call breaks open, ) dedents, } dangles (divergence) -->
	{#each a.filter((item)=>item.a&&item.b&&item.c&&item.d&&item.e&&item.f&&item.g&&item.h&&item.i) as item}{item}{/each}

	<!-- binary chain expression: wraps, } dangles (divergence) -->
	{#each aaaaaaaaaaaaaaaa&&bbbbbbbbbbbbbbbb&&cccccccccccccccc&&dddddddddddddddd&&eee as item}{item}{/each}

	<!-- 2-group member chain, head + body == 100: fits fully inline -->
	{#each a.filter((item)=>item.x).filter((item)=>item.yyyyyyyyyyyyyyyyyyy) as item}{item}{/each}

	<!-- 2-group member chain, head + body > 100 but head alone <= 100: head stays flat, body expands (one-pass; head-alone not wrapped) -->
	{#each a.filter((item)=>item.x).filter((item)=>item.yyyyyyyyyyyyyyyyyyyy) as item}{item}{/each}

	<!-- 2-group member chain, head alone > 100: head wraps, clause + } drop to base, body expands (divergence) -->
	{#each a.filter((item)=>item.x).filter((item)=>item.yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy) as item}{item}{/each}

	<!-- 3+ group member chain: always wraps regardless of width (divergence) -->
	{#each a.filter((item)=>item.x).map((item)=>item.y).filter((item)=>item.z) as item}{item}{/each}

	<!-- {:else} fallback, middle-zone (head fits alone, construct overflows): body, {:else}, fallback, and {/each} each drop to their own line (divergence) -->
	{#each fn00000(aaaaaaaaaaaaaaaaaaa,bbbbbbbbbbbbbbbbbbb,cccccccccc) as item}{item}{:else}fallback{/each}
</div>
