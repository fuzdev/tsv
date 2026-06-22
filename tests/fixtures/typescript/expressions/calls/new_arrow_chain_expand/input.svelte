<script>
	// Single arg: new expression with arrow returning arrow - block body
	new A(() => () => {
		const x = 1;
	});

	// Single arg: new expression with arrow returning arrow - object body
	new A(() => () => ({
		x: 1,
		y: 2
	}));

	// Single arg: arrow chain with call body - does NOT expand
	new A(() => () => fn(x));
</script>
