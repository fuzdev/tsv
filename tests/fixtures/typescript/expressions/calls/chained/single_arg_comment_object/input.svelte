<script>
	// Single arg inline block comment + plain object: expand-last guard
	const a = very_long_name
		.then((x) => x)
		.then((x) => x)
		.other(/* comment */ {a: 1, b: 2, c: 3, d: 4, e: 5});

	// Single arg inline block comment + array
	const b2 = very_long_name
		.then((x) => x)
		.then((x) => x)
		.other(/* comment */ [a, b, c, d, e, f, g, h]);

	// Single arg inline block comment + ternary
	const c = very_long_name
		.then((x) => x)
		.then((x) => x)
		.other(
			/* comment */ a > 0 ? very_long_true_value_abcdefghij : very_long_false_value_abcdefghij,
		);

	// Contrast: same without comment - ternary hugs
	const d2 = very_long_name
		.then((x) => x)
		.then((x) => x)
		.other(a > 0 ? very_long_true_value_abcdefghijklmno : very_long_false_value_abcdefghijklmno);
</script>
