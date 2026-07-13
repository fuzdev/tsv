<script>
	let n = $state('div');
</script>

{#snippet s()}{#await p}load{:catch e}<svelte:element this={n}
		></svelte:element>{/await}{/snippet}{@render s()}
