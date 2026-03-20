<script>
	// Nested ternary with line comment before consequent - inner breaks
	const a = cond1
		? // comment
			cond2
			? b
			: c
		: d;

	// Without comment - stays inline
	const b = cond1 ? (cond2 ? expr1 : expr2) : expr3;

	// In object property with line comment
	const obj = {
		prop: cond1
			? // comment
				cond2
				? b
				: c
			: d,
	};

	// Object property without comment - stays inline
	const obj2 = {
		prop: cond1 ? (cond2 ? b : c) : d,
	};

	// Trailing comment on nested consequent
	const c = cond1
		? cond2
			? b
			: c // trailing
		: d;

	// Line comment before alternate only - inner consequent still breaks
	const e = cond1
		? cond2
			? b
			: c
		: // comment
			d;

	// Multiline template literal in test - inner consequent breaks
	const f = `line1
line2`
		? cond2
			? b
			: c
		: d;
</script>
