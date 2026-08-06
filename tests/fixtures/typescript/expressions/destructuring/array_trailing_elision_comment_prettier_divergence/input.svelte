<script>
	// Past the last binding the run slides to the pattern's trailing position
	const [a, ,/* c */] = arr;

	// With no binding at all
	const [, ,/* c */] = arr;

	// An own-line comment takes its own line there, still past the elision comma
	const [
		b,
		,
		/* c */
	] = arr;
</script>
