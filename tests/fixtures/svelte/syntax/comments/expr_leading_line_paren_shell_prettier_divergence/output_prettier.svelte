{// c
a.b}
{@html // c
a.b}
{@render // c
a.b()}
{#if // c
a.b}
	x
{/if}
{#each // c
a.b as item}
	{item}
{/each}
{#key // c
a.b}
	x
{/key}
{#if cond}
	{@const y =
		// c
		a.b}
	{y}
{/if}
<div
	data-attr={// c
	a.b}
></div>
