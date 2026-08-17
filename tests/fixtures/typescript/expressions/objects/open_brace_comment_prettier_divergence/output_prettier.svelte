<script>
	// line comment trailing `{` — single property
	const o1 = {
		// c1
		a: 1
	};

	// line comment trailing `{` — multiple properties
	const o2 = {
		// c2
		a: 1,
		b: 2
	};

	// block comment trailing `{`, then own-line leading comment
	const o3 = {
		/* brace */
		// lead
		a: 1
	};

	// block comment trailing `{`, first property directly after
	const o4 = {
		/* brace2 */
		a: 1
	};

	// an author blank BELOW the pulled comment survives — the blank is authorship, not
	// the container's leading gap
	const o5 = {
		// c5

		a: 1
	};

	// the same, with the block spelling (an object literal preserves its authored
	// multi-line-ness, so a block on the `{` line is pulled with no `//` to force it)
	const o6 = {
		/* brace3 */

		a: 1
	};

	// the blank ABOVE the comment is a different question and stays erased — it sits
	// against the delimiter, where both formatters drop it at every bracket
	const o7 = {
		// c7
		a: 1
	};
</script>
