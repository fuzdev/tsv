<script>
	// The glued run fits at exactly 100 chars, so it rides the operator's line.
	const a1 = !(
		/* c1 */ /* c2 */
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	);

	// One char wider, the parens break and the run keeps the line the author gave it.
	const a2 = !(
		/* c1 */ /* c2 */
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	);

	// At the same width, a run the author glued to the operand travels with it.
	const a3 = !(/* c1 */ /* c2 */ bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);
</script>
