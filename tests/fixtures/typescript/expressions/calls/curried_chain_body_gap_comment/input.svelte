<script>
	// A curried chain in a call ARGUMENT whose terminal `=>`→body gap holds a glued block
	// comment. The chain layout emits that gap itself, so the author's redundant parens
	// around the body do not change the layout.
	foo(
		({ a }) =>
			(y) =>
				/* c1 */ ({ k: 1 })[0]
	);

	// the ternary body, whose layout parens the run rides inside
	foo(
		({ a }) =>
			(y) =>
				/* c2 */ p ? q : r
	);

	// the sequence body, which prints its own pair
	foo(
		({ a }) =>
			(y) => /* c3 */ (z, w)
	);

	// a hugging object body, which keeps the `=>` line
	foo(({ a }) => (y) => /* c4 */ ({ k: 1 }));

	// a block body
	foo(({ a }) => (y) => /* c5 */ {
		return z;
	});
</script>
