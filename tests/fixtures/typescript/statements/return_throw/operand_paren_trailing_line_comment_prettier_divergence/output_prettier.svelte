<script>
	// A `//` written inside a `return`/`throw` grouping paren forces the parenthesized
	// form on its own, with no leading comment to force it — the same layout
	// `hanging_paren_trailing_comment` reaches through a leading own-line comment.

	// a plain operand
	function fn1() {
		return x; // c
	}

	// throw is restricted the same way
	function fn2() {
		throw x; // c
	}

	// deferring past the `;` would merge the two comments into one
	function fn3() {
		return x; // c1 // c2
	}

	// a call operand
	function fn4() {
		return fn(); // c
	}

	// an assignment operand renders bare — the hanging parens are its clarity pair
	function fn5() {
		return (a = b); // c
	}

	// a ternary operand
	function fn6() {
		return cond ? a : b; // c
	}

	// a `throw` sequence operand, whose own printer floats the comment out
	function fn7() {
		throw (a, b); // c
	}

	// a comment past the closing paren is outside the parens, so it trails the `;`
	function fn8() {
		return x; /* t */ // c
	}
</script>
