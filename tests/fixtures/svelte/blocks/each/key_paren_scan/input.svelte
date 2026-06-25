{#each items as item, i (keys[')'])}
	<div>{i}</div>
{/each}
{#each items as item (getKey(item))}
	<div>{item}</div>
{/each}
