<script>
	async function fn1() {
		// The parens are redundant here, and retained for the comment's sake.
		await x /* c */;

		// A line comment keeps its own line inside the retained shell.
		await y; // c

		// Deferring the `//` past the `;` would merge it with the one already there.
		await z; // c1 // c2

		// A run the author wrote on its own line inside the parens: the block pair
		// collapses onto the operand's line (a trailing block's line is pure layout).
		await x /* c1 */ /* c2 */;

		// Two own-line line comments keep two lines - gluing them would make the second
		// one text inside the first.
		await x;
		// c1
		// c2

		// An own-line block and the line comment glued to it keep one line.
		await x;
		/* c1 */ // c2
	}

	function* fn2() {
		// `yield` binds looser than `+`, so even a binary operand's parens are redundant.
		yield a + b /* c */;

		// The line spelling, likewise retained.
		yield a; // c

		// The delegate form answers alike.
		yield* a /* c */;

		// An assignment operand's clarity parens are the same single pair.
		yield (a ??= b) /* c */;
	}
</script>
