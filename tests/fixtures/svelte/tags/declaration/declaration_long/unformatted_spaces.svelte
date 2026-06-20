<!-- root-level declaration tag that fits on one line (100 chars) -->
{const   a   =   item_variant && !['aaaa', 'bbbb', 'cccc', 'dddd', 'eeeeeeeeeeee'].includes(item_variant)}
{a}


<!-- root-level declaration tag that wraps at = (101 chars triggers break) -->
{let   b   =   item_variant && !['aaaa', 'bbbb', 'cccc', 'dddd', 'eeeeeeeeeeeeeee'].includes(item_variant)}
{b}


<!-- declaration tag that fits on one line (100 chars) -->
{#each items as item}
		{const   a   =   item_variant && !['aaaa', 'bbbb', 'cccc', 'dddd', 'eeeeeeeeee'].includes(item_variant)}
	{a}
{/each}


<!-- declaration tag that wraps at = (101 chars triggers break) -->
{#each items as item}
		{let   a   =   item_variant && !['aaaa', 'bbbb', 'cccc', 'dddd', 'eeeeeeeeeeeee'].includes(item_variant)}
	{a}
{/each}
