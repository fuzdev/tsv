<script>
	// Block comment glued to `=` with the value on the next line: the newline
	// collapses and the initializer stays inline.
	const a = /* c */
		b;

	// Own-line comment (a newline precedes it) keeps the break after `=`.
	const d =
		/* c */
		e;
</script>
