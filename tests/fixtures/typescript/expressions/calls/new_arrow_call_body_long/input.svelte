<script lang="ts">
	// 100 chars - fits on one line
	const a = new Xy((x) => fn(x, aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa));

	// 101 chars - breaks after =>
	const b = new Xy((x) =>
		fn(x, aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa)
	);

	// with await - 101 chars
	const c = await new Xy((x) =>
		fn(x, aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa)
	);
</script>
