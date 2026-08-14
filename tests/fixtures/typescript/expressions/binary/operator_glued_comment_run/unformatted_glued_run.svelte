<script>
	// A pair the author glued onto one line leads the operand inline
	const a =
		x +
		/* c1 */ /* c2 */
		y;

	// Logical operator - the same gap
	const b =
		x &&
		/* c1 */   /* c2 */
		y;

	// A glued run ending in a line comment still breaks
	const c =
		x + // c3
		/* c1 */   /* c2 */
		y;

	// Width still breaks the chain when the glued run does not fit
	const d = xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx + /* c1 */ /* c2 */ yyyyyyyyyyyyyyyyyyyyyyyyyyyyyy + zzzzzzzzzzzzzzzzzzzzzzzzzzzzzz;

	// Control - comments the author gave separate lines keep them, and the chain breaks
	const e =
		x +
		/* c1 */
		/* c2 */
		y;
</script>
