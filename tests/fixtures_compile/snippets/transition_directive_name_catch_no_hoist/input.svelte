<script>
	let fade = $state(() => {});
</script>

{#snippet s()}{#await p}load{:catch e}<div transition:fade></div>{/await}{/snippet}{@render s()}
