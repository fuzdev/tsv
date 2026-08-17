<script>
	// line comment trailing `[` — single element
	const a1 = [
		// c1
		x
	];

	// line comment trailing `[` — multiple elements
	const a2 = [
		// c2
		x,
		y
	];

	// block comment trailing `[`, then own-line leading comment
	const a3 = [
		/* bracket */
		// lead
		x
	];

	// an author blank BELOW the pulled comment survives — the blank is authorship, not
	// the container's leading gap (the blank ABOVE one stays erased, against the delimiter)
	const a4 = [
		// c3

		x
	];
</script>
