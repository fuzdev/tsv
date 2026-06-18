<script lang="ts">
	// A non-last argument with a block comment after the comma and a line comment
	// after it: the block stays on the comma line where it was written, the line
	// comment trails the comma.
	fn(
		a /* c1 */, // c2
		b,
	);

	// The after-comma block+line in the middle of three args (not just the first).
	fn(
		a /* c1 */, // c2
		b,
		c,
	);
</script>
