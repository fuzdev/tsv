<script>
	// Own-line block comment between elements forces expansion
	const [
		a,
		/* c */
		b,
	] = arr;

	// Own-line block comment before first element forces expansion
	const [
		/* c */
		a1,
		b1,
	] = arr;

	// Multiple own-line block comments between elements
	const [
		a2,
		/* c1 */
		/* c2 */
		b2,
	] = arr;

	// Same-line block comment does NOT force expansion (short destructuring)
	const [a3, /* c */ b3] = arr;

	// Inline block comment after comma does NOT force expansion
	const [a4, /* c */ b4, c4] = arr;
</script>
