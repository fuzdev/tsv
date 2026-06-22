<script lang="ts">
	// Own-line block comment between elements forces expansion
	type A = [
		string,
		/* c */
		number
	];

	// Own-line block comment before first element forces expansion
	type B = [
		/* c */
		string,
		number
	];

	// Multiple own-line block comments between elements
	type C = [
		string,
		/* c1 */
		/* c2 */
		number
	];

	// Same-line block comment does NOT force expansion (short tuple)
	type D = [string, /* c */ number];

	// Inline block comment after comma does NOT force expansion
	type E = [string, /* c */ number, boolean];
</script>
