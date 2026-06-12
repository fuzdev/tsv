<script>
	// Own-line block comment between elements forces expansion
	const a = [
		x,
		/* c */
		y,
	];

	// Own-line block comment before first element forces expansion
	const b = [
		/* c */
		x,
		y,
	];

	// Multiple own-line block comments between elements
	const c = [
		x,
		/* c1 */
		/* c2 */
		y,
	];

	// Same-line block comment does NOT force expansion (short array)
	const d = [x, /* c */ y];

	// Inline block comment after comma does NOT force expansion
	const e = [x, /* c */ y, z];
</script>
