<script lang="ts">
	// A block comment the author left between a statement's CONTENT and that statement's
	// own `;` belongs to the gap BETWEEN the two statements, not to the statement it
	// follows: the `;` is a pure separator, so the comment leads the next statement and,
	// being glued to that separator, shares its line.

	import a from './a';
	/* c1 */ fn1();

	// an expression statement
	fn2();
	/* c2 */ fn3();

	// a variable declaration
	const b = 1;
	/* c3 */ fn4();

	// a do/while
	do {} while (cond);
	/* c4 */ fn5();

	// a glued run travels whole, each comment answering its own glue
	fn6();
	/* c5 */ /* c6 */ fn7();
	fn8();
	/* c7 */
	/* c8 */ fn9();

	// a multi-line block hugs on its closing line
	fn10();
	/** c9
	 */ fn11();

	// a blank the author left ABOVE the comment survives, and one left BELOW it rides
	// above instead — the break it sat on is gone
	fn12();

	/* c10 */ fn13();
	fn14();

	/* c11 */ fn15();

	function fn16() {
		// a return statement
		return 1;
		/* c12 */ fn17();
	}

	function fn18() {
		// a throw statement
		throw e;
		/* c13 */ fn19();
	}

	for (;;) {
		// break, continue and debugger end at their keyword
		break;
		/* c14 */ continue;
		/* c15 */ debugger;
		/* c16 */ fn20();
	}

	// CONTROL: a comment that ENDS its line takes its own line either way — the glue
	// question is asked of what follows the comment, not of the `;`
	fn21();
	/* c17 */
	fn22();

	// CONTROL: a line comment can never glue to the `;` — its own line ends first
	fn23();
	// c18
	fn24();

	// CONTROL: a type-literal member is not a `;`-terminated STATEMENT, so the comment
	// stays inside the member and keeps its own line
	type T = {
		a: 1;
		/* c19 */
		b: 2;
	};

	// CONTROL: a class field is not one either
	class C {
		a = 1;
		/* c20 */
		b = 2;
	}
</script>
