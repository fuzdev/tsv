<script>
	let { items } = $props();
</script>

{#each items as item}
	{@const label = item}
	<li>{label}</li>
{/each}
