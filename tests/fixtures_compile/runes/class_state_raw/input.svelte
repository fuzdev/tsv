<script>
	class Store {
		data = $state.raw({ x: 1 });
	}
	const s = new Store();
</script>

<p>{s.data.x}</p>
