<script>
	let n = $state(true);
</script>

{#snippet s()}{#await p}load{:catch e}<div class:x={n}></div>{/await}{/snippet}{@render s()}
