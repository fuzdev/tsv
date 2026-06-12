<script>
	let a = $state(1);
	let b = $state(2);

	async function add(x, y) {
		return x + y;
	}
</script>

<p>{a} + {b} = {await add(a, b)}</p>
