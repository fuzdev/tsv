<script>
	// Chain (2+ calls) with arrow expression body call + trailing comment - args must break
	const a = a.b().fn1(() =>
		fn2(
			a,
			b // comment
		)
	);

	// Chain with own-line block comment
	const c = a.b().fn1(() =>
		fn2(
			a,
			/* comment */
			b
		)
	);

	// Chain with leading line comment between args
	const d = a.b().fn1(() =>
		fn2(
			a,
			// comment
			b
		)
	);

	// Short control - single member chain, no bug
	const e = obj.fn1(() =>
		fn2(
			a,
			b // comment
		)
	);
</script>
