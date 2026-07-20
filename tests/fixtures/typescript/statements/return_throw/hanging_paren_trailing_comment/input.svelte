<script>
	// A comment trailing the operand inside a `return`/`throw` grouping paren stays INSIDE
	// the parens, on the line the author gave it. The leading own-line comment is what forces
	// the parenthesized form — both are restricted productions (`[no LineTerminator here]`),
	// so a bare break after the keyword would be ASI rather than layout.

	// a same-line block comment trails the operand
	function fn1() {
		return (
			// c
			a /* t */
		);
	}

	// a line comment trails the operand
	function fn2() {
		return (
			// c
			a // t
		);
	}

	// an own-line block comment keeps its own line, still inside the parens
	function fn3() {
		return (
			// c
			a
			/* t */
		);
	}

	// a leading block comment forces the break the same way
	function fn4() {
		return (
			/* c */
			a ?? b /* t */
		);
	}

	// throw is restricted the same way
	function fn5() {
		throw (
			// c
			a /* t */
		);
	}

	// a ternary operand behaves the same
	function fn6() {
		throw (
			// c
			cond ? a : b /* t */
		);
	}

	// a comment past the closing paren is outside the parens, so it trails the `;` instead
	function fn7() {
		return (
			// c
			a
		); /* t */
	}

	// throw closes the same way
	function fn8() {
		throw (
			// c
			a
		); /* t */
	}
</script>
