<script>
	// a block after the comma stays on the comma line, in front of the line comment the
	// author wrote after it
	const a = { p1: (
1
), /* c1 */ // c2
p2: 2 };

	// a run of blocks after the comma keeps its order
	const b = { p1: (
1
), /* c1 */ /* c2 */ // c3
p2: 2 };

	// blocks on both sides of the comma each keep their side of it
	const c = { p1: (
1
)/* c0 */, /* c1 */ // c2
p2: 2 };

	// a block with no line comment after it leads the next property instead - nothing
	// defers, so it keeps the side of the comma the author chose
	const d = { p1: (
1
), /* c1 */ p2: 2 };
</script>
