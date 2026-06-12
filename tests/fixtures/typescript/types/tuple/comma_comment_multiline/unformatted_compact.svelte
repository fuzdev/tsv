<script lang="ts">
	// Inline block comment after comma with line comment forcing multiline
	type A = [
		string,
		// line comment
		/* c */number,
	];

	// Trailing block comment before comma with line comment
	type B = [
		string/* c */,
		// line comment
		number,
	];

	// Both trailing and leading around comma with line comment
	type C = [
		string/* c1 */,
		// line comment
		/* c2 */number,
	];
</script>
