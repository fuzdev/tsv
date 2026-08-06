<script>
	// A block comment after the comma leads the next element - it stays there
	const [a1, /* c */
b1] = arr;

	// An object pattern takes the same rule
	const { a2, /* c */
b2 } = obj;

	// Assignment-target position takes it too
	[a3, /* c */
b3] = arr;

	// Contrast - a comment before the comma trails the previous element
	const [a4 /* c */, b4] = arr;
	const { a5 /* c */, b5 } = obj;

	// A line comment expands the pattern - the glued comment still leads its element
	const [
		a6,
		/* c1 */ b6, // c2
		c6
	] = arr;
</script>
