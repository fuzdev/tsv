<script>
	let { items } = $props();
</script>

{#each items as item, i}
	<li>{i}: {item}</li>
{:else}
	<p>empty</p>
{/each}
