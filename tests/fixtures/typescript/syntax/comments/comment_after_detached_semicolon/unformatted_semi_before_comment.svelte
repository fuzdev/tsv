<script lang="ts">
	// A comment the author wrote AFTER a statement's own `;`, on the terminator's own
	// line, trails that statement: the `;` prints back up on the content's line, so the
	// comment shares the statement's last PRINTED line even though the source wrote it a
	// line below the content.

	import a1 from './a'
	; // c1

	// an expression statement
	fn1()
	; /* c2 */
	fn2();

	// a variable declaration
	const b1 = 1
	; // c3
	fn3();

	// the last statement of a block body
	function fn4() {
		fn5()
		; /* c4 */
	}

	// a return statement
	function fn6() {
		return 1
		; // c5
	}

	// a switch consequent with a statement following
	switch (cond) {
		case 1:
			fn7()
			; // c6
			fn8();
		case 2:
			fn9();
	}

	// past a RUN of dropped `;`s the comment still reaches the terminator's line
	fn10()
	;;;; // c7
	fn11();

	// with the gap's own comment as well: it trails on the CONTENT's line and closes
	// it, so the terminator line's comment takes a line of its own
	fn12() // c8
	;;;; // c9
	fn13();

	// a block on the terminator's line past the run
	fn14() // c10
	;;; /* c11 */
	fn15();

	// CONTROL: a comment written ABOVE the `;` is the gap's and leads the next statement
	fn16()
	/* c12 */;
	fn17();

	// CONTROL: a comment on its own line BELOW the `;` leads the next statement too
	fn18()
	;
	/* c13 */ fn19();
</script>
