<script>
	async function fn() {
		return 'value';
	}

	let val = $derived(await fn());
</script>

<p>{val}</p>
