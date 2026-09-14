<script>
	// a cast's own pair groups the sequence — no second pair inside it
	const a1 = /** @type {A} */ (b, c);
	x = /** @type {A} */ (b, c);
	fn(/** @type {A} */ (b, c));
	const a2 = /** @type {A} */ (b, c).d;
	function fn1() {
		return /** @type {A} */ (b, c);
	}

	// a multi-line cast comment takes the same single pair
	const a3 = /** @type {A}
	 */ (b, c);

	// the pair's edge gaps are the cast's: a comment after the last operand stays inside
	// the pair, and one on the first operand's line leads the run
	const a4 = /** @type {A} */ (b, (c /* t */));
	const a5 = /** @type {A} */ (
		// c
		b, c
	);
	const a6 = /** @type {A} */ (b, c /* t */);
</script>
