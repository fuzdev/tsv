<script lang="ts">
	// Inline block comment after comma with line comment forcing multiline
	type A = Map<
		string,
		// line comment
		/* c */ number
	>;

	// Trailing inline block comment before comma with line comment
	type B = Map<
		string /* c */,
		// line comment
		number
	>;

	// Both trailing and leading around comma with line comment
	type C = Map<
		string /* c1 */,
		// line comment
		/* c2 */ number
	>;
</script>
