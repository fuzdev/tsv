<script>
	// A block comment sharing its line with the next statement's start leads it.
	a1(); /* c1 */ let b1 = 1;

	// A glued pair leads together, on the next statement's line.
	a2(); /* c2 */ /* c3 */ let b2 = 2;

	// Same before an expression statement.
	a3(); /* c4 */ /* c5 */ b3();

	// A comment that ends its authored line trails the previous statement.
	a4(); /* c6 */
	let b4 = 4;

	// A run whose line ends after it trails too.
	a5(); /* c7 */ /* c8 */
	let b5 = 5;

	// The same rule in a block body; a multiline block reads its closing line.
	function fn1() {
		a6(); /* c9
		c10 */ const b6 = 6;
	}

	// A glued run whose tail is a multi-line block still leads.
	a7(); /* c11 */ /* x1
	y1 */ b7();

	// A run ending on its own after a multi-line block trails whole.
	a8(); /* x2
	y2 */ /* c12 */
	let b8 = 8;
</script>
