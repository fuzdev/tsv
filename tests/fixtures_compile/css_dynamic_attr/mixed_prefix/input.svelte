<script>
	let c = $state(true);
</script>

<p data-x="pre-{c ? 'x' : 'y'}">x</p>

<style>
	[data-x^='pre-'] {
		color: red;
	}
</style>
