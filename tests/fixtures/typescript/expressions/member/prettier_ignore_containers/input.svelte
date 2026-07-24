<script>
	// a format-ignored member slice is layout-opaque: each container keeps its
	// normal (tight) layout around the frozen multi-line slice

	// sole call argument
	f1(fn(2.0000)
		// prettier-ignore
		.c);

	// last of several arguments
	f2(a, b, fn(2.0000)
		// prettier-ignore
		.c);

	// new-expression argument
	new C1(fn(2.0000)
		// prettier-ignore
		.c);

	// ternary consequent
	const x = cond ? fn(2.0000)
		// prettier-ignore
		.c : y;

	// logical RHS
	const z = a && fn(2.0000)
		// prettier-ignore
		.c;

	// template interpolation
	const s = `t1${fn(2.0000)
		// prettier-ignore
		.c}t2`;
</script>
