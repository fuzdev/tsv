<script>
	// At the start of the body, where the run has no previous statement to break from.
	/* x0
	y0 */ a0();

	// A multi-line block glued to the next statement adds no blank line above the run.
	a1();
	// c1
	/* x1
	y1 */ b1();

	// The same after an own-line block comment.
	a2();
	/* c2 */
	/* x2
	y2 */ b2();

	// A blank line the author wrote above the run is still preserved.
	a3();
	// c3

	/* x3
	y3 */ b3();

	// The same rules inside a block body.
	function fn1() {
		// c4
		/* x4
		y4 */ a4();

		b4();
		// c5
		/* x5
		y5 */ c4();
	}
</script>
