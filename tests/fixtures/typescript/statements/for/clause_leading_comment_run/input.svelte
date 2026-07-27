<script>
	// A block glued to `(` leads the init clause without breaking the header
	for (/* a */ x = 0; b; c++) {}

	// A blank line between two own-line comments is preserved
	for (
		x = 0;
		// b1

		// b2
		b;
		c++
	) {}
</script>
