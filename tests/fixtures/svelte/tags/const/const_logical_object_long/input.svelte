<!-- Logical || with object RHS - object self-expands, = stays inline -->
{#each items as item}
	{@const a = item || {
		prop1: 'aaaaaaaaaaaaa',
		prop2: 'bbbbbbbbbbbbb',
		prop3: 'ccccccccccccc',
		prop4: 'dddddddddddd',
	}}
	{a}
{/each}
