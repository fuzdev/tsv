<script>
	// inline-adjacent block comment between properties stays inline
	const o1 = { a, /* c */ b };

	// own-line block comment between properties expands
	const o2 = {
		a,
		/* c */
		b
	};

	// line comment between properties expands
	const o3 = {
		a,
		// d
		b
	};

	// blank line around an own-line comment is preserved
	const o4 = {
		a,

		/* c */

		b
	};
</script>
