<!-- @const that fits on one line (100 chars) -->
{#each items as item}
	{@const a = item_variant && !['aaaa', 'bbbb', 'cccc', 'dddd', 'eeeeeeeee'].includes(item_variant)}
	{a}
{/each}

<!-- @const that wraps at = (101 chars triggers break) -->
{#each items as item}
	{@const a =
		item_variant && !['aaaa', 'bbbb', 'cccc', 'dddd', 'eeeeeeeeee'].includes(item_variant)}
	{a}
{/each}
