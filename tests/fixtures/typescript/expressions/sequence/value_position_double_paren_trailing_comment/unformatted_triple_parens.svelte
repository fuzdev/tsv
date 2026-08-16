<script lang="ts">
	// A DOUBLY-parenthesized sequence puts the author's comment between the two `)`.
	// Every redundant shell collapses into the sequence's own required pair, so a
	// comment in the collapsed region is emitted inside it.
	const a = (((x, y)) /* t */);

	b = (((x, y)) /* t */);

	const c = () => (((x, y)) /* t */);

	const d = (e = (((x, y)) /* t */));

	// Several comments — one gap or two depths — collapse in source order.
	const f = (((x, y /* t1 */)) /* t2 */);

	// Past the OUTERMOST `)` the gap is the statement terminator's, not the pair's.
	const h = (((x, y)) /* t1 */) /* t2 */;

	// A `for` header's init declarator answers the same at its clause separator.
	for (let i = (((x, y)) /* t */); ; ) {}

	// A line comment breaks the sequence and stays on the last operand.
	const g = () => (((x, y)) // t
	);

	// A `return` argument is a value position too, and hangs its operands.
	function fn1() {
		return (((x, y)) /* t */);
	}

	function fn2() {
		return (((x, y)) // t
		);
	}
</script>
