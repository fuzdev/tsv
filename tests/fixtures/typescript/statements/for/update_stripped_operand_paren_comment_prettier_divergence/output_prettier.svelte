<script lang="ts">
	// a comment written last inside a redundant pair down the update clause's right edge stays
	// in the header after the update, ahead of a comment written after the pair
	for (
		;
		;
		a + b // c1
	)
	// d1
	{}

	for (
		;
		;
		a + b // c2
	)
	/* d2 */
	{}

	for (; ; a + b)
	// c3
	// d3
	{}

	for (
		;
		;
		a + (b + c) // c4
	)
	// d4
	{}

	for (
		;
		;
		i++, a + b // c5
	)
	// d5
	{}

	for (
		i = 0;
		i < n;
		i = i + j // c6
	)
	// d6
	{}

	// a multi-line block takes the same place
	for (
		;
		;
		a + b /* c7
		 */
	)
	// d7
	{}

	// with nothing after the pair, the comment trails the update as before
	for (
		;
		;
		a + b // c8
	) {}

	for (
		;
		;
		a && b // c9
	)
	// d9
	{}

	// in an operand or declarator that another follows, the comment trails its comma
	for (
		;
		;
		i++,
			a + b, // c10
			j++ // d10
	) {}

	for (
		i = 0,
			k = a + b, // c11
			j = 1;
		;
	) {}

	for (
		let i = 0,
			k = a + b, // c12
			j = 1; // d12
		;
	) {}

	// the last declarator's value keeps its comment as before
	for (
		let k = a + b; // c13
		;
	) {}

	// a pair around a whole declarator value is the header's own shell, kept as before
	for (
		let i = 0,
			k = b, // c14
			j = 1;
		;
	) {}

	// a pair the printer adds ahead of a keyword, or an index, holds the comment, which
	// stays ahead of the one after it
	for (
		;
		;
		(a + b) as T // c15
	)
	// d15
	{}

	for (
		;
		;
		f(a)[b + c] // c16
	)
	// d16
	{}

	// a block written after a sequence's last pair follows the comment too
	for (
		;
		;
		i++, a + b // c17
	)
	/* d17 */
	{}
</script>
