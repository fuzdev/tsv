<script>
	// Head arg is call with multiline array — will_break, last arg is arrow
	fn1(
		new Map([
			['a', 1],
			['b', 1]
		]),
		(a, b) => (a[0] > b[0] ? -1 : 1)
	);

	// Head arg is call with callback — will_break, last arg is object
	fn2(
		fn3({ a: 1, b: 2, c: 3 }, (v) => v > 1),
		{ b: 2, c: 3 }
	);

	// Head arg is short expression arrow — does NOT will_break, stays inline
	fn4((v) => ({ a: `a_${v}`, b: `b_${v}` }), ['x', 'y', 'z']);
</script>
