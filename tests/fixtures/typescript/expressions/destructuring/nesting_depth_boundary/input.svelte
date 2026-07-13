<script>
	// Function-parameter context: expands at nesting depth 3+ (Prettier). Depth 2
	// stays inline; depth 3 expands.
	function f1({ a: { b } }) {}
	function f2({
		a: {
			b: { c }
		}
	}) {}

	// Standalone context: expands at nesting depth 2+. Depth 1 stays; depth 2 expands.
	const { d } = x;
	const {
		e: { f }
	} = y;

	// Defaults (`= …`) do NOT count toward the depth — a defaulted nested pattern
	// stays inline in both contexts, at any nesting.
	const { g: { h } = {} } = z;
	const { i: { j: { k } } = {} } = w;
	function f3({ l: { m } = {} }) {}
	function f4({ n: { o: { p } } = {} }) {}
</script>
