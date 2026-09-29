<script>
	// a `//` inside the required pair around a chain's LAST operand stays inside it,
	// and the pair opens around it
	const a =
		x -
		(
			y - z // c1
		);

	// the same at any depth: a call argument, a parameter default
	f(
		x -
			(
				y - z // c2
			),
		1
	);

	function g(
		v = x -
			(
				y - z // c3
			)
	) {
		return v;
	}

	// a logical operand and the exponent
	const b =
		x &&
		(
			y || z // c4
		);
	const d =
		x **
		(
			y ** z // c5
		);

	// a comment the author gave its own line keeps it, inside the pair
	const h =
		x -
		(
			y - z
			/* c6 */
		);

	// a comment after the statement stays its own comment
	const i =
		x -
		(
			y - z // c7
		); // c8

	// a `//` glued to the pair's `(` keeps the `(` line; one on a line of its own leads
	// the operand inside the pair
	const k =
		x -
		( // c10
			y - z // c11
		);
	const l =
		x -
		(
			// c12
			y - z // c13
		);

	// a pair the printer adds for clarity is not the author's: the comment belongs to the
	// shell around the whole operand
	const j = !(
		a || (b && c) // c9
	);
</script>
