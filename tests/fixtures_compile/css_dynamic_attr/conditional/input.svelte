<script>
	let c = $state(true);
</script>

<p data-x={c ? 'abc' : 'xyz'}>x</p>

<style>
	[data-x^='ab'] {
		color: red;
	}
</style>
