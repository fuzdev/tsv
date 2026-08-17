<script>
	// If - a line comment the author glued to the header `(` keeps that line
	if ( // c1
		a
	) {
		b();
	}

	// While - same
	while ( // c2
		a
	) {
		b();
	}

	// Do-while - same
	do {
		b();
	} while ( // c3
		a
	);

	// Switch - same, in the discriminant parens
	switch ( // c4
		a
	) {
		case 1:
			break;
	}

	// Catch - same, in the parameter parens
	try {
		b();
	} catch ( // c5
		e
	) {}

	// An author blank below the glued comment survives it
	if ( // c6

		a
	) {
		b();
	}

	// Do-while answers the blank the same way
	do {
		b();
	} while ( // c7

		a
	);

	// Only the glued comment claims the line - the rest of the run keeps its own
	if ( // c8
		// c9
		// c10
		a
	) {
		b();
	}

	// A block glued to the `(` claims nothing, so the whole run stays below it
	if (
		/* c11 */ // c12
		a
	) {
		b();
	}

	// Control - written on its own line the comment keeps its own line, and
	// prettier agrees
	if (
		// c13
		a
	) {
		b();
	}

	// Control - a block comment collapses inline in both formatters
	if (/* c14 */ a) {
		b();
	}
</script>
