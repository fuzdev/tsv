<script>
	// A line comment the author glued to a cast's `(` keeps that line, at `await`
	await /** @type {A} */ ( // d1
		x1
	);

	// and at a declarator.
	const v2 = /** @type {A} */ ( // d2
		x2
	);

	// Only the glued comment claims the line - the rest of the run keeps its own.
	const v3 = /** @type {A} */ ( // d3
		// e3
		x3
	);

	// An author blank below the glued comment survives it,
	const v4 = /** @type {A} */ ( // d4

		x4
	);

	// stopping at a nested cast's comment, above which none is added.
	const v5 = /** @type {A} */ ( // d5

		/** @type {B} */ (x5)
	);

	const v6 = /** @type {A} */ ( // d6
		/** @type {B} */ (x6)
	);

	// A block glued to the `(` claims nothing, so the whole run stays below it.
	const v7 = /** @type {A} */ (
		/* e7 */ // d7
		x7
	);

	// Control - written on its own line the comment keeps its own line.
	const v8 = /** @type {A} */ (
		// d8
		x8
	);
</script>
