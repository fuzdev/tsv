<script>
	// long chain - breaks at chain points, await stays flat
	const a = (await fn(arg, { withTypes: true }))
		.filter((x) => x.isValid())
		.map((x) => x.value)
		.sort();

	// medium chain (4 groups) - force expansion from callback count
	const b = (await fn(url))
		.json()
		.then((x) => x.items)
		.catch(() => []);

	// short chain - fits on one line
	const c = (await fn()).map((x) => x);
</script>
