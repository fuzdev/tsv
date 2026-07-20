<script>
	// If - blank between two own-line comments in the condition parens
	if (
		a

		// c1

		// c2
	) {
	}

	// While - same, in the condition parens
	while (
		a

		// c1

		// c2
	) {}

	// Switch - same, in the discriminant parens
	switch (a) {
		// c1

		// c2
		case 1:
			break;
	}

	// Catch - same, in the parameter parens
	try {
	} catch (
		e

		// c1

		// c2
	) {}

	// Block comments in the condition parens
	if (
		a

		/* c1 */

		/* c2 */
	) {
	}

	// If - blank between two own-line comments BEFORE the condition
	if (
		// c1

		// c2
		a
	) {
	}

	// Catch - same, before the parameter
	try {
	} catch (
		// c1

		// c2
		e
	) {}

	// Call arguments - already preserved (control)
	fn(
		a,

		// c1

		// c2
		b
	);
</script>
