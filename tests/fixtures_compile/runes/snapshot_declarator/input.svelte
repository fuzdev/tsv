<script>
	let obj = $state({ a: 1 });
	const s = $state.snapshot(obj);
</script>

{s.a}
