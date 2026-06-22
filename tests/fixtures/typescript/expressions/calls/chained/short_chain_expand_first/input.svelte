<script>
	// 2-group chain: expand-first with block callback + short second arg
	const a = b.method().other(
		(x) => {
			return x;
		},
		{ a: 1 }
	);

	// Same with longer base
	const c = very_long_base_name.method().other(
		(x) => {
			return x;
		},
		{ a: 1 }
	);

	// Contrast: 3-group chain works
	const e = b
		.method()
		.method2()
		.other(
			(x) => {
				return x;
			},
			{ a: 1 }
		);

	// Contrast: single member call works (not a chain)
	const g = b.other(
		(x) => {
			return x;
		},
		{ a: 1 }
	);
</script>
