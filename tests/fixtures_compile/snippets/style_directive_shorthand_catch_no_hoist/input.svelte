<script>
	let color = $state('red');
</script>

{#snippet s()}{#await p}load{:catch e}<div style:color></div>{/await}{/snippet}{@render s()}
