<script>
	// A comment in the gap between two heads leads the head after it, on its own line, and
	// the chain breaks after `=`
	const lineComment =
		(x) =>
		// c
		(y) =>
			z;
	const ownLineBlock =
		(x) =>
		/* c */
		(y) =>
			z;

	// A run stays in order, each comment on its own line; an authored blank line survives
	const run =
		(x) =>
		// c1
		// c2
		(y) =>
			z;
	const runBlank =
		(x) =>
		// c1

		// c2
		(y) =>
			z;

	// A block the author glued to the line comment stays glued to it
	const mixedRun =
		(x) =>
		/* c1 */ // c2
		(y) =>
			z;

	// The gap one head further along reads the same way
	const secondGap =
		(x) =>
		(y) =>
		// c
		(w) =>
			z;

	// A block-statement terminal reads the same way
	const blockBody =
		(x) =>
		// c
		(y) => {
			go();
		};

	// Control: a single-line block glued to the next head forces nothing - the chain stays flat
	const glued = (x) => /* c */ (y) => z;

	// A multiline block travels with the head it is glued to, and its own break opens the chain
	const gluedMultiline =
		(x) =>
		/* c1
	c2 */ (y) =>
			z;
</script>
