<script lang="ts">
	// A non-last argument with a block comment before the comma and a line comment
	// after it: the block trails the argument, the line comment trails the comma.
	// Neither is dropped.
	fn(
		a /* c1 */, // c2
		b,
	);

	// Two block comments before the comma, then a line comment after it: all kept.
	fn(
		a /* c1 */ /* c2 */, // c3
		b,
	);

	// The block+line argument in the middle of three args (not just the last).
	fn(
		a /* c1 */, // c2
		b,
		c,
	);
</script>
