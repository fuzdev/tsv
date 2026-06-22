<script>
	// Two consecutive line comments before first arg
	fn(
		// comment 1
		// comment 2
		a,
		b
	);

	// Three consecutive line comments before first arg
	fn(
		// comment 1
		// comment 2
		// comment 3
		a
	);

	// Mixed: line then block comment before first arg
	fn(
		// comment 1
		/* comment 2 */
		a,
		b
	);

	// Single line comment before first arg (baseline - should work)
	fn(
		// comment
		a,
		b
	);
</script>
