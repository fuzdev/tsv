<script>
	let { name } = $props();
</script>

{#snippet s()}<svelte:element this={name}>x</svelte:element>{/snippet}{@render s()}
