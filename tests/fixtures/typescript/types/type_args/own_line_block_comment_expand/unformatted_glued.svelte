<script lang="ts">
	// Own-line block comment between type arguments forces expansion
	type A = Map<
		string,
		/* c */
		number
	>;

	// Own-line block comment before first argument forces expansion
	type B = Map<
		/* c */
		string,
		number
	>;

	// Multiple own-line block comments between arguments
	type C = Map<
		string,
		/* c1 */
		/* c2 */
		number
	>;

	// A same-line block comment does NOT force expansion
	type D = Map<string,
/* c */number>;
</script>
