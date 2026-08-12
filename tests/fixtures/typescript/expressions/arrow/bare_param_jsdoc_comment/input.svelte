<script>
	// A `/** @type */` comment glued to a BARE parameter is not a cast: a cast is the
	// comment plus the `(` it leads, and the next token here is the parameter itself.
	// The parens that parameter gains are the arrow's parameter LIST, so the comment
	// still leads the arrow.
	const a = /** @type {any} */ (x) => x;
	const b = /** @satisfies {A} */ (x) => x;

	// The cast for contrast — the `(` follows the comment in source, and it is kept
	const c = /** @type {any} */ (x);
</script>
