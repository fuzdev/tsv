<script>
	// line comment trailing `{` — single property
	const o1 = { // c1
		a: 1
	};

	// line comment trailing `{` — multiple properties
	const o2 = { // c2
		a: 1,
		b: 2
	};

	// block comment trailing `{`, then own-line leading comment
	const o3 = { /* brace */
		// lead
		a: 1
	};
</script>
