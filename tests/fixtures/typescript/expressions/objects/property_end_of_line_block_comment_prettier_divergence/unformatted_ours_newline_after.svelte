<script>
	// a block comment after the comma leads the next property - it stays there
	const a = { property1: 1, /* c */
property2: 2 };

	// contrast - a comment before the comma trails the previous property
	const b = { property1: 1 /* c */, property2: 2 };

	// a comment given a line of its own expands the object
	const c = {
		property1: 1,
		/* c */
		property2: 2
	};
</script>
