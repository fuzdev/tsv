<script>
	// The spread's stripped-paren comment slides past the elision, leading the next element
	const a = [
		...b,
		/* c */
		,
		d
	];

	// Past two elisions
	const e = [
		...f,
		/* c */
		,
		,
		g
	];

	// The run keeps its authored order
	const h = [
		...i,
		/* c1 */
		/* c2 */
		,
		j
	];

	// The spread need not be the first element
	const k = [
		l,
		...m,
		/* c */
		,
		n
	];
</script>
