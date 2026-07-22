<script>
	let props = $state({});
	let x = $state('a');
</script>

<input type="radio" bind:group={x} value="a" {...props} />
