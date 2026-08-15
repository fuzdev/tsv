<script>
	// A run in the binding→keyword gap keeps each comment on its own line; the
	// keyword still takes its continuation indent.
	for (
		a // c1
		/* c2 */
			of
			arr
	)
		expr;

	// A run the author glued onto one line keeps that line.
	for (
		b /* c1 */ /* c2 */ // c3
			of
			arr
	)
		expr;

	// A blank line between two comments in the gap is preserved.
	for (
		c // c1

		/* c2 */
			of
			arr
	)
		expr;

	// So is one the author left between the binding and the run's first comment.
	for (
		m

		// c1
			of
			arr
	)
		expr;

	// A multiline block after a line comment takes its own line.
	for (
		d // c1
		/* c2
		c3 */
			of
			arr
	)
		expr;

	// The keyword→iterable gap answers the same way (for-in shares the path).
	for (
		e
			in // c1
			/* c2 */
			obj
	)
		expr;

	// So does the iterable→`)` gap.
	for (
		f
			of
			arr // c1
			/* c2 */
	)
		expr;

	// An own-line comment in the binding→keyword gap keeps its line once another
	// gap's line comment breaks the header.
	for (
		g
		/* c1 */
			of // c2
			arr
	)
		expr;

	// The `(`→binding gap is the fourth: a run the author glued to the binding
	// leads it on one line.
	for (
		/* c1 */ /* c2 */ h // c3
			of
			arr
	)
		expr;

	// A blank line between two own-line comments there is preserved too.
	for (
		// c1

		// c2
		k // c3
			of
			arr
	)
		expr;
</script>
