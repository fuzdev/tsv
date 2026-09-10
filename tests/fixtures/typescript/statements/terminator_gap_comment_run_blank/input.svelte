<script lang="ts">
	// a comment trailing the content, an author blank, then a comment in the gap
	fn1(); // c1

	// c2
	fn2();

	// blocks in both positions
	fn3(); /* c3 */

	/* c4 */
	fn4();

	// a run below the blank
	fn5(); // c5

	// c6
	// c7
	fn6();

	// CONTROL: the run spans the terminator — the blank is the run's own
	fn7(); // c8

	// c9
	fn8();

	// CONTROL: no trailing comment on the content's line
	fn9();

	// c10
	fn10();

	// CONTROL: an explicit terminator on the content's line
	fn11(); // c11

	// c12
	fn12();
	// a block body, a namespace body and a switch consequent take the same rule
	function fn13() {
		fn14(); // c13

		// c14
		fn15();
	}

	namespace N1 {
		fn16(); // c15

		// c16
		fn17();
	}

	switch (cond) {
		case 1:
			fn18(); // c17

			// c18
			fn19();
		case 2:
			fn20();
	}
</script>
