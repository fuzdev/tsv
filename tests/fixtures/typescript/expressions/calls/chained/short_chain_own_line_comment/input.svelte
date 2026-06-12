<script>
	// Short chain (2-group) with own-line line comment in last call
	const a = b.method().other(
		// comment
		(x) => handle(x, a, b, c, d),
	);

	// Short chain (2-group) with own-line block comment in last call
	const b2 = b.method().other(
		/* comment */
		(x) => handle(x, a, b, c, d),
	);

	// Short chain with own-line comment + plain object
	const c = b.method().other(
		// comment
		{a: 1, b: 2, c: 3, d: 4, e: 5},
	);

	// Short chain with own-line comment + multiple args
	const d2 = b.method().other(
		// comment
		a,
		b,
		c,
	);
</script>
