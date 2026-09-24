<script>
	// Past the last real element the run slides to the array's trailing position
	const a = [x /* c */, ,];

	// An own-line comment takes its own line there, still past the elision comma
	const b = [
		x,
		/* c */
		,
	];

	// Two comments there keep a space between them, inline and on their own line
	const c = [x /* c1 */ /* c2 */, ,];

	const e = [
		x,
		/* c1 */ /* c2 */
		,
	];
</script>
