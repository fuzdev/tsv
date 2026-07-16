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

	// Line comment, own-line block, then a multiline block glued to the first arg —
	// all on consecutive lines (no blank before the glued block).
	fn(
		// comment 1
		/* comment 2 */
		/* comment 3
		 */ a
	);

	// A glued block run before the first arg stays glued (not split), even with a
	// multiline block glued to the arg.
	fn(
		// comment 1
		/* comment 2 */ /* comment 3
		 */ a
	);
</script>
