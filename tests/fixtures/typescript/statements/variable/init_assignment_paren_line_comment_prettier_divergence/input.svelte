<script lang="ts">
	// A line comment trailing a parenthesized assignment operand in a value position
	// stays inside the parens, on the operand's line.
	const y = (
		a = b // c1
	);

	// A leading own-line comment stays adjacent to the value it leads — the broken
	// parens introduce no blank line between them.
	const z =
		// c2
		(
			c = d // c3
		);
</script>
