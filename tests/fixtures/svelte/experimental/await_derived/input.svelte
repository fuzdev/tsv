<script>
	async function fetchData() {
		return 'data';
	}

	let data = $derived(await fetchData());
</script>

<p>{data}</p>
