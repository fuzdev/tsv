<!-- Nested ternary - test stays on same line as = -->
{#each items as item}
	{@const a = cond
		? 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
		: cond
			? 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'
			: 'cccccccccccccccccc'}
	{a}
{/each}
