<script>
	// A C-style for header IS a ForStatement, so an init clause chain stays flush with
	// the clause (prettier's shouldNotIndent names it; the for-in/of iterable is not one)
	for (
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ??
		cccccccccccccccccccccccccccccccccccccccccccccc ??
		ddd;
		;
	) {}

	// The update clause is the same term
	for (
		;
		;
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ??
		cccccccccccccccccccccccccccccccccccccccccccccc ??
		ddd
	) {}

	// A sequence clause's ELEMENTS are not that term — their parent is the sequence — so an
	// element chain takes the continuation indent, one level past the clause
	for (
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ??
			cccccccccccccccccccccccccccccccccccccccccccccc ??
			ddd,
			bb;
		;
	) {}

	// The sequence clause's own indent starts AFTER the first operand, so a first operand
	// that breaks internally keeps its lines at the clause column
	for (
		fnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn(
			aaaaaaaaaaaaaaaaaaaaaaaaaaaa,
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		),
			bb;
		;
	) {}

	// 100 chars - the header stays inline
	for (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ?? ccccccccccccccccccccccccccccccccccc ?? ddd; ;) {}

	// 101 chars - the header breaks onto its own lines; the clause itself still fits
	for (
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ?? cccccccccccccccccccccccccccccccccccc ?? ddd;
		;
	) {}
</script>
