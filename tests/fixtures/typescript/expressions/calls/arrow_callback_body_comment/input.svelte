<script>
	// Trailing line comment forces inner call to break - outer ) must be separate
	const a = fn1(() =>
		fn2(
			a,
			// @ts-expect-error
			b,
		),
	);

	// Own-line block comment forces inner call to break - same pattern
	const c = fn1(() =>
		fn2(
			a,
			/* comment */
			b,
		),
	);

	// Trailing comment on last arg
	const d = fn1(() =>
		fn2(
			a,
			b, // comment
		),
	);

	// Short control - no comment, fits inline
	fn1(() => fn2(a, b));
</script>
