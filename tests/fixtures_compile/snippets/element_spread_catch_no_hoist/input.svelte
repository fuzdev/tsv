<script>
	let n = $state({ a: 1 });
</script>

{#snippet s()}{#await p}load{:catch e}<div {...n}></div>{/await}{/snippet}{@render s()}
