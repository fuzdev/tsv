<script lang="ts">
	// long chain (4+ calls with complex args) - force expansion
	const a = (await fn(arg, {withTypes: true}))!
		.filter((x) => x.isValid())
		.map((x) => x.value)
		.sort();

	// medium chain (4 groups with callbacks) - force expansion
	const b = (await fn(url))!
		.json()
		.then((x) => x.items)
		.catch(() => []);

	// short chain - fits on one line
	const c = (await fn())!.map((x) => x);
</script>
