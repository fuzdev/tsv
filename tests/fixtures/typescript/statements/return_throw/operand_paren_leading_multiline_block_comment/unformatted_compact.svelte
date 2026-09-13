<script>
	// A leading block comment that CONTAINS a line terminator forces the break by itself: a
	// MultiLineComment holding one counts as a line terminator (ECMA-262 §12.4), so the
	// hanging parens are what keep `return` / `throw` — restricted productions — out of ASI.
	function fn1() {
		return (/* a
b */ a);
	}

	// the argument's shape does not change the answer — every arm takes the same pair
	function fn2() {
		return (/* a
b */ a.b);
	}
	function fn3() {
		return (/* a
b */ a());
	}
	function fn4() {
		return (/* a
b */ cond ? a : b);
	}
	function fn5() {
		return (/* a
b */ a, b);
	}
	function fn6() {
		return (/* a
b */ a = b);
	}

	// the remaining argument shapes, whose left-side siblings diverge only in how the shell is
	// AUTHORED (see operand_paren_leading_multiline_block_left_side_prettier_divergence): on
	// this form the two formatters agree at both (the non-null sibling is TS-only, so it
	// rides the divergence fixture's lang="ts" script instead)
	function fn2a() {
		return (/* a
b */ a`t`);
	}
	function fn2c() {
		return (/* a
b */ a.b.c());
	}

	// a binary argument reaches the same form through its own conditional pair
	function fn7() {
		return (/* a
b */ a + b);
	}

	// the control: a single-line block comment spans no line, so nothing forces a break and
	// the redundant parens drop
	function fn8() {
		return /* c */ a;
	}

	// the pair's claim on the comment is what keeps the ternary flat, and it does not depend
	// on the comment forcing the break: the own-line `//` forces it here, and the block the
	// argument's first token owns still prints outside the ternary's group
	function fn9() {
		return (
			// c
			/* a
b */ cond ? a : b);
	}

	// throw is restricted the same way
	function fn10() {
		throw (/* a
b */ a);
	}
</script>
