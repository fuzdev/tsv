<script>
	// a block comment glued to a stripped paren shell's closer trails the element
	[a1 /* c */, b1] = x;

	// a line comment in the same position defers past the comma
	[
		a2, // c
		b2
	] = x;

	// a block then a line comment each keep the side of the comma the author gave them
	[
		a3 /* c1 */, // c2
		b3
	] = x;

	// an object pattern's property covers the closer, so the run trails it the same way
	const { a4 = 1 /* c */, b4 } = x;

	// an own-line block comment with nothing before it on its line leads the next element
	[a5, /* c */ b5] = x;

	// the same shell at the LAST element, where the bracket closes the run instead of a comma
	[a6, b6 /* c */] = x;
</script>
