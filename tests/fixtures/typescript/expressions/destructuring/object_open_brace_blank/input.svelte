<script>
	// a blank line after an object pattern's `{` is dropped - the pattern collapses
	const { a1 } = o;

	// an arrow parameter takes the same rule
	const fn1 = ({ a2 }) => a2;

	// a function declaration parameter too
	function fn2({ a3 }) {
		return a3;
	}

	// and a catch binding
	try {
		fn2(o);
	} catch ({ a4 }) {}

	// an assignment pattern is a pattern position too
	const fn3 = ({ a5 } = o) => a5;

	// a nested pattern hugged by a parameter stays inline - the blank adds no reason to expand
	const fn4 = ({ a6: { b6 } }) => b6;

	// contrast - an object literal keeps the expansion, only patterns drop it
	const o1 = {
		a7: 1
	};

	// contrast - a blank line between a pattern's properties is preserved
	const {
		a8,

		b8
	} = o;
</script>
