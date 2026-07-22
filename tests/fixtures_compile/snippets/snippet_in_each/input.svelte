<script>
	let { items } = $props();
</script>

{#snippet row(item)}
	<li>{item}</li>
{/snippet}

<ul>
	{#each items as item}
		{@render row(item)}
	{/each}
</ul>
