<script>
	let c = $state(true);
</script>

<p data-x={c || 'a'}>x</p>

<style>
	[data-x='a'] {
		color: red;
	}
</style>
