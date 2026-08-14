<script>
	// A pair the author glued onto one line leads the condition inline
	if (
		/* c1 */ /* c2 */
		x
	) {
		a();
	}

	// While - the same gap
	while (
		/* c1 */ /* c2 */
		x
	) {
		b();
	}

	// Do-while - the same gap through its own emitter
	do {
		c();
	} while (
		/* c1 */ /* c2 */
		x
	);

	// Switch - the discriminant parens
	switch (
		/* c1 */ /* c2 */
		x
	) {
		case 1:
			break;
	}

	// Width still opens the parens when the glued run does not fit
	if (/* c1 */ /* c2 */ aaaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbbb && cccccccccccccccccccccccccc) {
		d();
	}

	// Control - comments the author gave separate lines keep them, and the parens open
	if (
		/* c1 */
		/* c2 */
		x
	) {
		e();
	}

	// Control - a line comment forces the break wherever it sits
	if (
		// c1
		x
	) {
		f();
	}
</script>
