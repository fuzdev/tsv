<script>
	// Inline: two JSDoc cast args fit on one line
	fn(/** @type {A} */ (aaaa), /** @type {B} */ (bbbb));

	// Args break: each cast stays with its expression
	fn(
		/** @type {AAAAAAAAA} */ (aaaaaaaaaaaa),
		/** @type {BBBBBBBBB} */ (bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)
	);

	// Three args with JSDoc casts
	fn(
		/** @type {AAAAAAAAAAAA} */ (aaaaaaaaaaaaa),
		/** @type {BBBBBBBBBBBB} */ (bbbbbbbbbbbbb),
		cccccccccccc
	);

	// Cast before a call expression arg
	fn(
		/** @type {AAAAAAAAAA} */ (call(aaaaaa, bbbbbb)),
		/** @type {BBBBBBBBBB} */ (call(cccccc, dddddd))
	);

	// JSDoc cast before non-first arg only
	fn(
		aaaaaaaaaaaaaaaaaaaaaaaa,
		/** @type {BBBBBBBBBBBBBBBBBBBBBBBBBBB} */ (bbbbbbbbbbbbbbbbbbbbbbbbb)
	);
</script>
