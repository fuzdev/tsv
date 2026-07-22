<script>
	let { rows } = $props();
</script>

<div>
	{#snippet cell(value)}
		<td>{value}</td>
	{/snippet}

	{#each rows as row}
		{@render cell(row)}
	{/each}
</div>
