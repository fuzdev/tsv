<script>
	// a spread's stripped parens leave an own-line comment behind, and elisions follow the
	// spread: the comment and the one written after the `)` travel past the elision commas
	// together, in the order the author wrote them
	const a1 = [
		...b /* t */,
		/* i */
		,
		c
	];

	// with only holes after the spread they reach the array's trailing position, ahead of a
	// line comment written there
	const a2 = [
		...b /* t */,
		/* i */ // h
		,
	];

	const a3 = [
		...b /* t */,
		/* i */ // h
		,
	];

	// a block written after the holes travels with them, each block its own space apart
	const a4 = [
		...b /* t */ /* h */,
		/* i */
		,
	];
</script>
