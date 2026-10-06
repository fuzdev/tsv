<script>
	// A block comment spanning lines that trails a binary `return`/`throw` operand inside the
	// operand's grouping parens stays inside them, and forces the parenthesized form.

	// a comment whose lines are kept as written
	function fn1() {
		return (
			a ?? b /* c1
			c2 */
		)
	}

	// a comment whose lines align on `*`
	function fn2() {
		return (
			a && b /**
			 * c3
			 */
		)
	}

	// throw is restricted the same way
	function fn3() {
		throw (
			a || b /* c4
			c5 */
		)
	}

	// an operand that breaks
	function fn4() {
		return (
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &&
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb /* c6
			c7 */
		)
	}

	// a method carrier
	const obj = {
		m() {
			return (
				this.a + this.b /* c8
				c9 */
			)
		}
	};
</script>
