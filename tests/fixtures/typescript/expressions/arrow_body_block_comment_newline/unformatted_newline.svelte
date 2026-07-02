<script>
	// Single-line block glued to `=>` with the body on the next line stays inline.
	const f = () => /* c */
		a;

	// Own-line comment keeps the break.
	const g = () =>
		/* c */
		b;
</script>
