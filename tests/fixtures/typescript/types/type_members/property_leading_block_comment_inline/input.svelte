<script lang="ts">
	// inline-adjacent block comment between members stays inline
	type T1 = { a: 1; /* c */ b: 2 };

	// own-line block comment between members expands
	type T2 = {
		a: 1;
		/* c */
		b: 2;
	};

	// line comment between members expands
	type T3 = {
		a: 1;
		// d
		b: 2;
	};

	// blank line around an own-line comment is preserved
	type T4 = {
		a: 1;

		/* e */

		b: 2;
	};
</script>
