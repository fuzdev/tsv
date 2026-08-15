<script>
	// an own-line line comment left behind by a spread's stripped parens keeps its own
	// line, a sibling in the parent list — like an own-line block, and in source order

	// array: with an interior own-line block and an after-comma block glued to the spread
	const a1 = [
		1,
		...x1 /* t */
		// c
		/* i */
	];

	// array: alone, on an element the list continues past
	const a2 = [
		1,
		...x2,
		// c
		b2
	];

	// call: last argument, with an interior own-line block
	fn(
		a,
		...x3
		// c
		/* i */
	);

	// call: block first — source order holds
	fn(
		a,
		...x4
		/* i */
		// c
	);

	// object literal: with an interior own-line block
	const o1 = {
		...x5,
		// c
		/* i */
		b5: 1
	};
</script>
