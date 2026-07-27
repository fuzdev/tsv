<script>
	// A comment in the empty test slot stays in that slot
	for (
		a;
		// c1
		;
		c++
	) {
		fn();
	}

	// A comment in the empty update slot stays in that slot
	for (
		a;
		b;
		// c2
	) {
		fn();
	}

	// A comment in the empty init slot stays in that slot
	for (
		// c3
		;
		b;
		c++
	) {
		fn();
	}

	// An own-line block comment keeps its own line, like the line spelling
	for (
		a;
		/* c4 */
		;
		c++
	) {
		fn();
	}

	// An inline block comment stays on the header line
	for (a; /* c5 */ ; c++) fn();
</script>
