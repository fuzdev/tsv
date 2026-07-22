<script>
	let { a } = $props();
	let d = $derived({ items: [] });
</script>

{#each d.items as x}{x}{/each}
