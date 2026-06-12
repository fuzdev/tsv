<script lang="ts">
	// Block comment after opening brace, member on next line: object breaks
	type A = {
		/* c */
		a: number;
	};

	// Block comment after opening brace, member on same line: stays inline
	type B = {/* c */ a: number};

	// Multiple members, comment after opening brace
	type C = {
		/* c */
		a: number;
		b: string;
	};

	// Nested object value, comment after inner opening brace
	type D = {
		nested: {
			/* c */
			a: number;
		};
	};
</script>
