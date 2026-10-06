<script>
	// A comment trailing a binary `return`/`throw` operand inside the operand's grouping
	// parens stays inside them, before the `)`.

	// a line comment forces the parenthesized form
	function fn1() {
		return (
			a && b // c1
		);
	}

	// throw is restricted the same way
	function fn2() {
		throw (
			a || b // c2
		);
	}

	// a block comment stays before the `)` of an operand that breaks
	function fn3() {
		return (
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &&
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb /* c3 */
		);
	}

	// a method carrier
	const obj = {
		m() {
			return (
				this.a && this.b // c4
			);
		}
	};

	// an arrow carrier
	const fn4 = () => {
		throw (
			a + b // c5
		);
	};

	// a comment past the `)` is outside the parens and trails the `;`
	function fn5() {
		return (
			a < b // c6
		); // c7
	}

	// an operand that breaks, with a line comment on its last line
	function fn6() {
		return (
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ||
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb // c8
		);
	}

	// a clause body
	function fn7() {
		if (cond)
			return (
				a && b // c9
			);
		else fn();
	}

	// a block comment and a line comment on the operand's line
	function fn8() {
		return (
			a && b /* c10 */ // c11
		);
	}

	// a statement follows, with and without a blank line
	function fn9() {
		throw (
			a || b // c12
		);
		fn();

		throw (
			a || b // c13
		);

		fn();
	}

	// a statement of the script itself
	throw (
		a || b // c14
	);
</script>
