<script>
	// Method chain with array of 3+ objects (2+ props each)
	// Prettier keeps the opening bracket hugged: `fn(a).b([`
	// We incorrectly expand to: `fn(a).b(\n  [\n    ...`
	fn(a).b([
		{aaaaaa: 'bbbbbbbbb', value: 2},
		{aaaaaa: 'bbbbbbbbb', value: 4},
		{aaaaaa: 'bbbbbbbbb', value: 6},
	]);

	// Same pattern with expect().toEqual() - common in tests
	expect(results).toEqual([
		{aaaaaa: 'bbbbbbbbb', value: 2},
		{aaaaaa: 'bbbbbbbbb', value: 4},
		{aaaaaa: 'bbbbbbbbb', value: 6},
	]);

	// 2 objects with 2+ props - works correctly (below threshold)
	fn(a).b([
		{aaaaaa: 'bbbbbbbbb', value: 2},
		{aaaaaa: 'bbbbbbbbb', value: 4},
	]);

	// 3+ objects with 1 prop each - works correctly (single prop objects inline)
	fn(a).b([{a: 1}, {b: 2}, {c: 3}]);
</script>
