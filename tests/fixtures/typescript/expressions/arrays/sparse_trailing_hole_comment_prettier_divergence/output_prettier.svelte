<script>
	// Past the last real element the run slides to the array's trailing position
	const a = [x /* c */, ,];

	// An own-line comment takes its own line there, still past the elision comma
	const b = [
		x,
		/* c */
		,
	];
</script>
