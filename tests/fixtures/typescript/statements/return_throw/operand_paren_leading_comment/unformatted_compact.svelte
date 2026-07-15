<script>
	// A leading comment on a parenthesized `return`/`throw` argument must not push the
	// argument onto the next line: both are restricted productions
	// (`[no LineTerminator here]`), so a break after the keyword means ASI. A line comment
	// keeps the parens and breaks inside them.
	function fn1() {
		return ( // c
		a);
	}

	// the argument shape doesn't matter — a member expression behaves the same
	function fn2() {
		return ( // c
		a.b);
	}

	// a ternary argument too
	function fn3() {
		return ( // c
		cond ? a : b);
	}

	// throw is restricted the same way
	function fn4() {
		throw ( // c
		a);
	}

	// a block comment with the argument after it on the same line collapses inline — nothing
	// forces a break, so no parens are needed
	function fn5() {
		return /* c */ (
			a
		);
	}

	// the same block comment followed by a newline does force a break, so it keeps the parens
	function fn6() {
		return ( /* c */
		a);
	}
</script>
