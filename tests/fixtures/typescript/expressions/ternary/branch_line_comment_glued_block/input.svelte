<script>
	// A branch gap whose run ends in a block the author GLUED to the value: the block and
	// the value keep the line the author wrote them on, below the `//` that took its own.

	// `?` branch
	const a = cond
		? // c1
			/* c2 */ x
		: y;

	// the `:` branch, same shape
	const b = cond
		? x
		: // c3
			/* c4 */ y;

	// control: the block on its own line keeps the value on the next
	const c = cond
		? // c5
			/* c6 */
			x
		: y;

	// control: two line comments, neither of which can share the value's line
	const d = cond
		? // c7
			// c8
			x
		: y;
</script>
