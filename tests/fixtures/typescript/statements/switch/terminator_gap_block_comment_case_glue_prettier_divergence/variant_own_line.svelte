<script>
	// a lone block comment in the last statement's terminator gap, before a sibling case
	switch (a1) {
		case 1:
			fn1();
		/* c1 */
		case 2:
			fn2();
	}
	// the same gap with `default` as the next label
	switch (a2) {
		case 1:
			fn3();
		/* c2 */
		default:
			fn4();
	}
	// the block as a run's LAST member, behind a `//` that already ended its own line
	switch (a3) {
		case 1:
			fn5();
		// c3
		/* c4 */
		case 2:
			fn6();
	}
	// CONTROL: the LAST case has no label to glue to, so the comment keeps its own line
	switch (a4) {
		case 1:
			fn7();
		/* c5 */
	}
	// CONTROL: a statement FOLLOWING in the same consequent takes the glue, at its level
	switch (a5) {
		case 1:
			fn8();
			/* c6 */ fn9();
		case 2:
			fn10();
	}
	// an author blank below the terminator rides ABOVE the glued run — the glue leaves no
	// line below the comment to carry it, and both formatters agree here
	switch (a6) {
		case 1:
			fn11();

		/* c7 */
		case 2:
			fn12();
	}
</script>
