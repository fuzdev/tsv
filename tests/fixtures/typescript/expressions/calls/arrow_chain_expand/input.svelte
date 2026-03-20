<script>
	// Multi-arg: arrow returning arrow with block body - expands last arg
	fn(a, () => () => {
		const x = 1;
	});

	// Multi-arg: arrow returning arrow with object body - expands last arg
	fn(a, () => () => ({
		x: 1,
		y: 2,
	}));

	// Multi-arg: arrow returning arrow returning arrow with block body (triple chain)
	fn(a, () => () => () => {
		const x = 1;
	});

	// Multi-arg: arrow returning arrow with call body - does NOT expand (arrowChainRecursion)
	fn(a, () => () => fn(x));

	// Multi-arg: arrow returning arrow with ternary body - does NOT expand
	fn(a, () => () => (x ? y : z));

	// Single arg: arrow returning arrow with block body - expands
	fn(() => () => {
		const x = 1;
	});

	// Single arg: arrow returning arrow with object body
	fn(() => () => ({
		x: 1,
		y: 2,
	}));

	// Multi-arg: three args with arrow chain last
	fn(a, b, () => () => {
		const x = 1;
	});
</script>
