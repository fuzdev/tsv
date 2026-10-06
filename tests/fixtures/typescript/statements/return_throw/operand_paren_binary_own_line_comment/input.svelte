<script>
	// A comment on its own line between a binary `return`/`throw` operand and the `)` of the
	// operand's grouping parens keeps that line, inside the parens.

	// a line comment
	function fn1() {
		return (
			a && b
			// c1
		);
	}

	// a block comment
	function fn2() {
		return (
			a && b
			/* c2 */
		);
	}

	// throw is restricted the same way
	function fn3() {
		throw (
			a || b
			// c3
		);
	}

	// a run: one comment trails the operand's line, the next owns a line
	function fn4() {
		return (
			a ?? b // c4
			// c5
		);
	}

	// a pair written on one line keeps that line
	function fn5() {
		return (
			a + b
			/* c6 */ /* c7 */
		);
	}

	// a blank line above the comment survives
	function fn6() {
		return (
			a < b

			// c8
		);
	}

	// a block comment spanning lines
	function fn7() {
		return (
			a | b
			/* c9
			c10 */
		);
	}

	// an operand that breaks
	function fn8() {
		return (
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &&
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
			// c11
		);
	}

	// a method carrier
	const obj = {
		m() {
			throw (
				this.a && this.b
				/* c12 */
			);
		}
	};

	// an arrow carrier
	const fn9 = () => {
		return (
			a instanceof b
			// c13
		);
	};

	// a clause body
	function fn10() {
		if (cond)
			return (
				a && b
				// c14
			);
		else fn();
	}

	// a comment past the `)` is outside the parens and trails the `;`
	function fn11() {
		return (
			a in b
			// c15
		); // c16
	}

	// a block comment trails the operand's line, the next comment owns a line
	function fn12() {
		return (
			a && b /* c17 */
			// c18
		);
	}

	// a statement follows after a blank line
	function fn13() {
		throw (
			a || b
			// c19
		);

		fn();
	}
</script>
