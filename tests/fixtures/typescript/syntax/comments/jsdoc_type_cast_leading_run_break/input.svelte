<script>
	// A run glued ahead of a cast does not take away the break the author left after the
	// cast's own comment — the separator is read from what follows that `*/`, not from
	// whether the comment starts its line.
	a1();
	/* c1 */ /** @type {A} */
	(b1);

	// With nothing glued ahead of it the answer is the same.
	a2();
	/** @type {A} */
	(b2);

	// Glued on both sides, the cast stays on one line.
	a3();
	/* c2 */ /** @type {A} */ (b3);

	// In a value gap the break collapses with the rest of the layout when it fits.
	const a4 = /* c3 */ /** @type {A} */ (b4);

	const a5 = /* c4 */ /** @type {A} */ (b5);

	// A cast the author isolated on its own line keeps it, and the value hangs under it.
	const a6 =
		/** @type {A} */
		(b6);

	// A call argument collapses like any other container that flattens when it fits.
	fn(/* c5 */ /** @type {A} */ (b7));
</script>
