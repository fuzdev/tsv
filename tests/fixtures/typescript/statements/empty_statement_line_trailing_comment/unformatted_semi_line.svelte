<script lang="ts">
	// A dropped `;` prints nothing, so it cannot begin a new output line: a comment
	// sharing its line trails the statement above it, exactly as one past a detached
	// terminator does.

	// one dropped `;` on its own line
	fn1();
	; // c1
	fn2();

	// a run of them, all on the comment's line
	fn3();
	;;; // c2
	fn4();

	// a block trailer
	fn5();
	; /* c3 */
	fn6();

	// the last statement of a block body
	function fn7() {
		fn8();
		; // c4
	}

	// a switch consequent
	switch (cond) {
		case 1:
			fn9();
			; // c5
		case 2:
			fn10();
	}

	// CONTROL: the `;`s on a line of their own, the comment a line further down — it
	// leads the next statement
	fn11();
	;;;
	// c6
	fn12();

	// CONTROL: a comment glued to the next statement past the `;` leads it
	fn13();
	; /* c7 */ fn14();

	// CONTROL: a comment BEFORE a dropped `;` on the statement's own line already
	// trails it
	fn15(); /* c8 */ ;
	fn16();
</script>
