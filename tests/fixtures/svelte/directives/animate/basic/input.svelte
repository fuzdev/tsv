{#each items as item (item.id)}
	<div animate:flip></div>
{/each}

{#each items as item (item.id)}
	<div animate:flip={{duration: 200}}></div>
{/each}
