<script>
	// a block comment after the last argument's comma trails the argument - the comma is
	// never re-emitted, so the comment keeps its line and the call stays inline
	fn(a /* c */);

	// the same with two arguments
	fn(a, b /* c */);

	// a new expression takes the same rule
	new A(a /* c */);

	// a multiline block comment trails the argument and expands the list
	fn(
		a /* c1
	   c2 */
	);

	// contrast - a comment the author gave a line of its own keeps it and expands the list
	fn(
		a
		/* c */
	);
</script>
