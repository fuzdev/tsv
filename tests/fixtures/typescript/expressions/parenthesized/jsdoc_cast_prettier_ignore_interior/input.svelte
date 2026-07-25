<script>
	// a directive alone on its line inside a JSDoc cast's own parens freezes the cast's
	// inner expression — the cast's comment and parens print around the frozen slice
	const a = /** @type {T} */ (
		// prettier-ignore
		fn(1,   2)
	);

	// the block spelling keeps its line here too: a glued directive is inert, so
	// collapsing it inline would lose the freeze on the next pass
	const b = /** @type {T} */ (
		/* prettier-ignore */
		fn(1,   2)
	);

	// as a call argument — the position the ignore ledger keys this on
	c.d(
		[e],
		/** @type {T} */ (
			// prettier-ignore
			f.g(1,   2)
		)
	);

	// an ordinary own-line comment in the same gap still collapses inline (no freeze,
	// nothing to preserve)
	const h = /** @type {T} */ (/* c */ fn(1, 2));

	// a block comment glued before the inner is owned by it and stays put
	const i = /** @type {T} */ (
		// prettier-ignore
		/* c */ fn(1,   2)
	);
</script>
