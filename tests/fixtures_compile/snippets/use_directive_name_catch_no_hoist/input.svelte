<script>
	let n = $state(() => {});
</script>

{#snippet s()}{#await p}load{:catch e}<div use:n></div>{/await}{/snippet}{@render s()}
