<script>
	// Chain call: multiline stripped parens — no spurious blank line
	a.b(calleeeee, left_computed ? left_prop : fn1(left_prop_name), right_value, fn1(locate_value));

	// Non-chain call: same pattern
	fn(calleeeee, left_computed ? left_prop : fn1(left_prop_name), right_value, fn1(locate_value));

	// Chain call nested: stripped parens inside inner call within ternary
	a.b(
		calleeeee,
		fn1(),
		cond
			? a.d(e_value, fn2(f_value.left), f_value.right)
			: a.e(e_value, fn2(f_value.argument), f_value.prefix),
		g_value
	);

	// Object property: stripped parens on value
	const obj = {
		x: b ? c : d,
		y: e
	};

	// Array element: stripped parens
	const arr = [
		bbbbbbbbbbb ? ccccccccccc : ddddddddddd,
		eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee,
		ffffffffffffffffffffffffffffffffffff
	];
</script>
