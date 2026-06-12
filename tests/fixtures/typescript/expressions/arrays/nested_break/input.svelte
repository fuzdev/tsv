<script>
	// breaks: 2+ elements, all arrays/objects, each with 2+ items
	const a = [
		[1, 2],
		[3, 4],
	];
	const b = [
		{a: 1, b: 2},
		{c: 3, d: 4},
	];

	// breaks with blank line: preserved between elements
	const g = [
		[1, 2],

		[3, 4],
	];
	const h = [
		{a: 1, b: 2},

		{c: 3, d: 4},
	];

	// inline: inner items have only 1 element
	const c = [[1], [2]];
	const d = [{a: 1}, {b: 2}];

	// inline: not all inner items have 2+ elements
	const e = [[1, 2], [3]];
	const f = [{a: 1, b: 2}, {c: 3}];
</script>
