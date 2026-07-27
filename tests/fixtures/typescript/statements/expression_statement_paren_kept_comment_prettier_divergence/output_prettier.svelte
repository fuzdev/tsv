<script>
	// A bare string keeps its parens so it isn't read as a directive; a comment
	// after the `(` breaks the parens open and keeps its place inside them
	/* c1 */
	('str');

	// An object at statement start keeps its parens
	/* c2 */
	({ a: 1 });

	// c3
	({ b: 2 });

	// A class or function expression at statement start keeps its parens too
	/* c4 */
	(class {});

	// c5
	(function () {});

	// A run the author glued onto one line stays glued
	/* c6 */ /* c7 */
	({ c: 3 });

	// A blank line between two comments is preserved
	// c8

	// c9
	({ d: 4 });
</script>
