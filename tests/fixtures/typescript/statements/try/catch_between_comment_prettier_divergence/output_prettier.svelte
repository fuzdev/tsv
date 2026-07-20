<script>
	// Own-line comment before catch/finally: keeps its own line
	try {
		a();
	} catch (e) {
		// c1
		b();
	} finally {
		// c2
		c();
	}

	// Own-line block comment: also keeps its own line
	try {
		a();
	} catch (e) {
		/* c3 */
		b();
	}

	// Trailing line comment: stays trailing, `catch` drops to the next line
	try {
		a();
	} catch (e) {
		// c4
		b();
	}

	// Trailing block comment: `catch` stays on the line
	try {
		a();
	} /* c5 */ catch (e) {
		b();
	}

	// A blank line above an own-line comment is preserved
	try {
		a();
	} catch (e) {
		// c6
		b();
	}

	// Mixed: a trailing comment keeps the brace line, an own-line one follows it
	try {
		a();
	} catch (e) {
		// c7
		// c8
		b();
	}
</script>
