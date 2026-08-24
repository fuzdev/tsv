<script>
	// An own-line line comment between `case` and its test keeps the line the author
	// gave it; the test hangs one level in, below the comment.
	switch (a) {
		case
		// c1
		x:
			b();
			break;
		// The test's parens are the printer's, so the comment has no node to ride inside.
		case
		// c2
		(y, z):
			b();
	}

	// A run stays in place, in order, each comment on its own line.
	switch (d) {
		case
		// c3
		// c4
		w:
			b();
			break;
		// A block ahead of the line comment keeps its own line too.
		case
		/* c5 */
		// c6
		v:
			b();
	}

	// An author blank between the comment and the test survives.
	switch (e) {
		case
		// c7

		u:
			b();
	}
</script>
