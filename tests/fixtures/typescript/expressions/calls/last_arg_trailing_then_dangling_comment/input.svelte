<script lang="ts">
	// Line comment trailing the last arg, then a line comment on its own line:
	// the first trails (after the comma), the second stays on its own line.
	fn(
		a,
		b // c1
		// c2
	);

	// Same, with three: only the first trails; the rest each get their own line.
	fn(
		a,
		b // c1
		// c2
		// c3
	);

	// Block comment then line comment, both on the last arg's line: the block
	// trails before the comma, the line trails after it (neither is dropped).
	fn(
		a,
		b /* c1 */ // c2
	);

	// Single arg: block + line on the arg's line.
	fn(
		a /* c1 */ // c2
	);

	// Block trailing the arg, then a block on its own line.
	fn(
		a,
		b /* c1 */
		/* c2 */
	);
</script>
