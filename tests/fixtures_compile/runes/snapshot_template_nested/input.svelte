<script>
	let obj = $state({ a: 1 });
	function wrap(x) {
		return x;
	}
</script>

{wrap($state.snapshot(obj))}
