<!-- Binary chain in @const - continuation lines at same indent as first operand -->
{#each items as item}
	{@const a =
		item +
		'aaaaaaaaaaaaaaaaaaaaaaaaa' +
		'bbbbbbbbbbbbbbbbbbbbbbbbb' +
		'cccccccccccccccccccc' +
		'dddddddddddddddddddddddd'}
	{a}
{/each}
