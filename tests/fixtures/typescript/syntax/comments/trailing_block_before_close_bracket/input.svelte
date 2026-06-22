<script lang="ts">
	// array pattern: trailing block comment before `]` on its own line forces expansion
	const [
		a
		/* c */
	] = arr;

	// tuple type: trailing block comment before `]`
	type T = [
		b
		/* c */
	];

	// array expression: trailing block comment before `]`
	const x = [
		c
		/* c */
	];
</script>
