<script>
	// A line comment between `case` and its test stays after the keyword; the test
	// drops one level in.
	switch (a) {
		case // c
		x:
			b();
			break;
		// The test's parens are the printer's, so the comment has no node to ride inside.
		case // c2
		(y, z):
			b();
	}

	// A run stays in place, in order, each comment distinct.
	switch (d) {
		case // c3
		// c4
		w:
			b();
			break;
		// A block ahead of the line comment keeps its place in the run.
		case /* c5 */ // c6
		v:
			b();
	}
</script>
