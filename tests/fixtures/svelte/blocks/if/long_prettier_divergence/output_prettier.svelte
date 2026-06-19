<div>
	<!-- fits inline (head + inline body <= 100): stays fully inline -->
	{#if a && b && c && d && e && f && g && h && i && j && k && l && m && n && o && p && rrrrrr}x{/if}

	<!-- binary chain head > 100: wraps, } dangles on its own line, body drops down (divergence) -->
	{#if a && b && c && d && e && f && g && h && i && j && k && l && m && n && o && p && rrrrrrrrrrrrr}
		x
	{/if}

	<!-- function-call operand in a binary chain: head wraps, } dangles (divergence) -->
	{#if a && fn(arg00000000, arg11111111, arg2222222222, arg333333333, arg444444444, arg55555555) && b}
		x
	{/if}

	<!-- {:else if} head > 100: wraps, } dangles (divergence) -->
	{#if a}
		x
	{:else if b && c && d && e && f && g && h && i && j && k && l && m && n && o && p && q && rrrrrrrr}
		y
	{/if}

	<!-- member chain (3+ groups): wraps, } dangles (divergence) -->
	{#if items
		.filter((x) => x.active)
		.map((x) => x.id)
		.includes(targetValueeeeeeeeeeeeeeeeeeeeeeeeee)}
		x
	{/if}

	<!-- single call whose args wrap: ) dedents to base, then } dangles on its own line (divergence) -->
	{#if someCondition(argument1xxxx, argument2xxxx, argument3xxxx, argument4xxxx, argument5xxxx, arg6)}
		z
	{/if}

	<!-- foo(...) at 100 cols: args fit inline on the {#if line; 3+ chain wraps; } dangles (divergence) -->
	{#if foo(afffffffffffffffffffffffffffffffffffff, bbbbbbbbbbbbbbbbbbbbb, ssssssssssssssssssssss, d)
		.bar()
		.baz()
		.fooooooooooooooooooooooooooooooooooooooo()}
		x
	{/if}

	<!-- nested arg breaks: foo(...) args break AND the last chain call's args break (at 101); } dangles (divergence) -->
	{#if foo(afffffffffffffffffffffffffffffffffffff, bbbbbbbbbbbbbbbbbbbbb, ssssssssssssssssssssss, dd)
		.bar()
		.baz()
		.fooooooooooooooooooooooooooooooooooooooo(aaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbb, ccccccccccccccccc)}
		x
	{/if}

	<!-- 2-group member chain at the boundary: fits fully inline -->
	{#if a.filter((item) => item.x).filter((item) => item.yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy)}x{/if}

	<!-- 2-group member chain, head alone > 100: wraps, } dangles, body expands (divergence) -->
	{#if a
		.filter((item) => item.x)
		.filter((item) => item.yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy)}
		x
	{/if}
</div>
