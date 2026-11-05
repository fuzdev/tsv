<script>
	// a comment written last inside a redundant pair down the right edge of a JSDoc cast's
	// inner stays inside the cast's pair, ahead of a comment written after the pair, each on a
	// line of its own
	k1 = /** @type {T} */ (
		a + b // c1
		// d1
	);
	let k2 = /** @type {T} */ (
		a + (b + c) // c2
		// d2
	);
	let k3 = /** @type {T} */ (
		a + b
		// c3
		// d3
	);
	fn1(
		/** @type {T} */ (
			a + b // c4
			// d4
		)
	);
	function fn2() {
		return /** @type {T} */ (
			a + b // c5
			// d5
		);
	}

	// an own-line comment after the pair already takes a line of its own
	k6 = /** @type {T} */ (
		a + b // c6
		// d6
	);

	// with nothing written after the pair, the comment stays inside the cast's pair too
	k7 = /** @type {T} */ (
		a + b // c7
	);
</script>
