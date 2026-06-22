<script lang="ts">
	// Inline block comment after comma with line comment forcing multiline
	type A = (
		a: string,
		// line comment
		/* c */ b: number
	) => void;

	// Trailing block comment before comma with line comment
	type B = (
		a: string /* c */,
		// line comment
		b: number
	) => void;

	// Both trailing and leading around comma with line comment
	type C = (
		a: string /* c1 */,
		// line comment
		/* c2 */ b: number
	) => void;
</script>
