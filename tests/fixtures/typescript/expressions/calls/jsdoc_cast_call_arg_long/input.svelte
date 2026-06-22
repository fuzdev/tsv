<script>
	// Inline: JSDoc cast wrapping a call arg, fits on one line
	fn(/** @type {Expression} */ (call(aaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbb, ccccccccccccccccc)));

	// Long: the cast's inner call expands inside the preserved parens
	fn(
		/** @type {Expression} */ (
			call(aaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbb, cccccccccccccccccccc)
		)
	);

	// Long with a preceding arg
	fn(
		aaaa,
		/** @type {Expression} */ (
			call(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)
		)
	);

	// Cast wrapping a ternary as a call arg
	fn(
		a,
		/** @type {Expression} */ (
			conddddd ? aaaaaaaaaaaaaaaaaaaaaaaaaaaa : bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		)
	);
</script>
