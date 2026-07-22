<script>
	let { items } = $props();
</script>

{#each items as item}
	<li>{item}</li>
{/each}
