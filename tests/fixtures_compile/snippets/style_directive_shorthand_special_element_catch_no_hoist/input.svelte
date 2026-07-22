<script>
	let color = $state('red');
</script>

{#snippet s()}{#await p}load{:catch e}<svelte:element this={"div"} style:color
		></svelte:element>{/await}{/snippet}{@render s()}
