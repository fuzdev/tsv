<script>
	// A block comment the author left trailing the operator stays after it
	const first = alpha + /* c1 */ beta;

	// Logical operator - the same gap
	const second = alpha && /* c1 */ beta;

	// A run whose second comment is a multi-line block: the block can't print flat, so the
	// run carries a hard break of its own wherever a formatter places it
	const third =
		alpha +
		/* c2 */ /* c3
		d3 */
		beta;
</script>
