<script>
	// a spread's stripped parens leave an own-line comment behind; a comment written after
	// the `)` follows it, in the order the author wrote them

	// a block after the `)` glues onto the inside comment's line, with or without a
	// trailing comma between them
	fn(
		a,
		...b /* t */
		/* i */
	);

	new A(
		a,
		...b /* t */
		/* i */
	);

	obj.method(
		a,
		...b /* t */
		/* i */
	);

	fn(
		a,
		...b /* t */
		/* i */
	);

	new A(
		a,
		...b /* t */
		/* i */
	);

	// a line comment inside the parens ends its line, so the comment after the `)` starts
	// the next one — a block or a line comment, before or after a trailing comma
	fn(
		a,
		...b /* t */
		// i
	);

	new A(
		a,
		...b /* t */
		// i
	);

	obj.method(
		a,
		...b /* t */
		// i
	);

	fn(
		a,
		...b /* t */
		// i
	);

	new A(
		a,
		...b /* t */
		// i
	);

	obj.method(
		a,
		...b /* t */
		// i
	);

	fn(
		a,
		...b
		// i // t
	);

	new A(
		a,
		...b
		// i // t
	);

	obj.method(
		a,
		...b
		// i // t
	);

	fn(
		a,
		...b
		// i // t
	);

	new A(
		a,
		...b
		// i // t
	);

	obj.method(
		a,
		...b
		// i // t
	);

	// the inside run may mix the two kinds
	fn(
		a,
		...b /* t */
		/* i */
		// c
	);

	// a line comment on the argument's line inside the parens ends that line too, so a block
	// after the `)` starts the next one, with or without a trailing comma between them
	fn(
		a,
		...b /* t */ // i
	);

	new A(
		a,
		...b /* t */ // i
	);

	obj.method(
		a,
		...b /* t */ // i
	);

	fn(
		a,
		...b /* t */ // i
	);

	new A(
		a,
		...b /* t */ // i
	);

	fn(
		a,
		...b /* t */ // i1
		// i2
	);

	// unchanged: a line comment after the `)`, and an own-line block below it
	fn(
		a,
		...b
		/* i */ // t
	);

	fn(
		a,
		...b
		/* i */
		/* t */
	);

	fn(
		a,
		...b // i
		/* t */
	);
</script>
