<script>
	// An alternate-nested ternary's test continues one level past the `:` plus two columns
	const a = !b
		? 'x'
		: c.d === eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee.f ||
			  c.d === eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee.g
			? 'y'
			: 'z';

	// A call in that test: arguments two levels past the `:`, the `)` at the test's own offset
	const b = c
		? 1
		: fn(
					aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
					bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
			  )
			? 2
			: 3;

	// A consequent-nested ternary's test continues one level past the `?`
	const c = d
		? eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee ||
			ffffffffffffffffffffffffffffffffffff ||
			gggggggggggggggggggggggg
			? 1
			: 2
		: 3;

	// A call in that test
	const d = e
		? fn(
				aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
				bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
			)
			? 2
			: 3
		: 4;

	// Under `return` the nested test takes the same flush chain at the same offset
	function fn1() {
		return a
			? 1
			: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ||
				  cccccccccccccccccccccccccccccccccccccc ||
				  ddddddddddddddddddddddddd
				? 2
				: 3;
	}

	// 100 chars - the nested test stays inline
	const f = g
		? 1
		: hhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhh || iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii
			? 2
			: 3;

	// 101 chars - breaks
	const h = g
		? 1
		: hhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhh ||
			  iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii
			? 2
			: 3;
</script>
