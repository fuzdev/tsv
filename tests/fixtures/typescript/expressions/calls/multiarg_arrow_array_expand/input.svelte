<script lang="ts">
	// 2-arg: arrow + array - expands all (penultimate arrow blocks expand-last)
	const a = fn(
		(v: string) => ({a: `aaaa_${v}`, b: `bbbbbbbb: ${v};`}),
		['aaaaaaa', 'bbbbbbb', 'ccccccc'],
	);

	// Short control - fits inline
	fn((v) => v, ['a', 'b']);

	// 3-arg: arrow not blocking - expand-last still works for array
	fn(aaaa, (v: string) => ({a: `aaaa_${v}`, b: `bbbbbbbb: ${v};`}), [
		'aaaaaaa',
		'bbbbbbb',
		'ccccccc',
	]);
</script>
