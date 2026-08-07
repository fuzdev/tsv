<script>
	// a block comment after the comma leads the next argument - it stays there
	fn(a, /* c */ b);

	// contrast - a comment before the comma trails the previous argument
	fn(a /* c */, b);

	// a comment given a line of its own expands the argument list
	fn(
		a,
		/* c */
		b
	);
</script>
