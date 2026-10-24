<script>
	// A cast's pair keeps the author's blank lines in its leading run, as prettier does
	// (a JavaScript script keeps a cast's parens): between two comments,
	await /** @type {A} */ (
		// a1

		// b1
		x1
	);

	// between a line comment and the value,
	await /** @type {A} */ (
		// a2

		x2
	);

	// and below an own-line block, which keeps its line rather than gluing onto the
	// line comment after it, with a blank
	await /** @type {A} */ (
		/* e3 */

		// d3
		x3
	);

	// or without one.
	await /** @type {A} */ (
		/* e4 */
		// d4
		x4
	);

	// A declarator takes the same run.
	const v1 = /** @type {A} */ (
		// a5

		// b5
		x5
	);

	const v2 = /** @type {A} */ (
		// a6

		x6
	);

	const v3 = /** @type {A} */ (
		/* e7 */

		// d7
		x7
	);

	const v4 = /** @type {A} */ (
		/* e8 */
		// d8
		x8
	);

	// A block the author kept on the line comment's line stays there,
	await /** @type {A} */ (
		/* e9 */ // d9
		x9
	);

	// a nested cast's own blank stays its own, with none added above its comment,
	await /** @type {A} */ (
		// a10
		/** @type {B} */

		(b10)
	);

	// and a directive keeps the blank below it.
	await /** @type {A} */ (
		// a11
		/* prettier-ignore */

		x11
	);
</script>
