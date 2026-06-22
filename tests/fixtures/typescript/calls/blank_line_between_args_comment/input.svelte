<script>
	// Blank lines between arguments with inter-argument comments preserved
	fn(
		// comment 1
		a,

		// comment 2
		b,

		// comment 3
		c
	);

	// Blank lines between args with trailing line comments
	fn(
		a, // trailing 1

		b, // trailing 2

		c
	);

	// Blank lines with block comments between args
	fn(
		/* comment 1 */
		a,

		/* comment 2 */
		b
	);

	// Mixed: some blank lines, some not
	fn(
		// comment 1
		a,
		// comment 2 (no blank line before)
		b,

		// comment 3 (blank line before)
		c
	);
</script>
