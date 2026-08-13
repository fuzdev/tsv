<script>
	// If - block comment sharing the closing paren's line
	if (a
	/* c1 */) {
		b();
	}

	// While - same
	while (a
	/* c2 */) {
		b();
	}

	// Do-while - same
	do {
		b();
	} while (a
	/* c3 */);

	// Catch - same, in the parameter parens
	try {
		b();
	} catch (e
	/* c4 */) {}

	// Run - a line comment trailing the condition, then a block on the closing paren's line
	if (a // c5
	/* c6 */) {
		b();
	}

	// Run - an own-line comment, then one sharing the closing paren's line
	if (a
	/* c7 */
	/* c8 */) {
		b();
	}
</script>
