<script>
	let { items } = $props();
</script>

{#each items as item}
	{#if item}
		<li>{item}</li>
	{/if}
{/each}
