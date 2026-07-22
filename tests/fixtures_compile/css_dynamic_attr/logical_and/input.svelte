<script>
	let c = $state(true);
</script>

<p data-x={c && 'v'}>x</p>

<style>
	[data-x=''] {
		color: red;
	}
</style>
