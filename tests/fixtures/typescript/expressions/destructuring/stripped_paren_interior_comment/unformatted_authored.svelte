<script>
	// a block comment inside a stripped paren shell trails the element it sits in
	const { a1 = (1 /* c */), b1 } = x;

	// the same shell on a renamed property
	({ a2: (b2 /* c */) } = x);

	// an array pattern element's default carries the same shell
	const [a3 = (1 /* c */)] = x;

	// the shell on the LAST element, where the closer ends the run instead of a comma
	const { b4, a4 = (1 /* c */) } = x;

	// a line comment in the shell defers past the comma and expands the pattern
	const { a5 = (1 // c
	), b5 } = x;
</script>
