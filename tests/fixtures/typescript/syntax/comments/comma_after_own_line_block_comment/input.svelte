<script>
	// a block comment the list's own comma follows on its line does not own that line, so
	// the list still flattens - the author's break around a comma is layout
	const a1 = [x1, /* c1 */ y1];

	// the same rule in every list family: the array pattern
	const [a2, /* c2 */ b2] = x2;

	// the object literal
	const a3 = { a: 1, /* c3 */ b: 2 };

	// the object pattern
	const { a4, /* c4 */ b4 } = x4;

	// call arguments
	fn1(a5, /* c5 */ b5);

	// a parameter list
	function fn2(a6, /* c6 */ b6) {}
</script>
