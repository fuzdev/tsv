<script>
	// Arrow operator `=>` inside expression after `,` in comparison RHS
	// scan_for_closing_angle_bracket must not treat `>` in `=>` as closing `>`
	const a = { '<': (a, b) => a < b, '<=': (a, b) => a <= b };

	// Object with operators as string keys
	const b = { '>=': (a, b) => a >= b, '>': (a, b) => a > b };

	// Arrow in array element after comparison
	const c = [a < b, (x) => x];

	// Arrow in function argument after comparison
	fn(a < b, (x) => x);

	// `<=` and `>=` operators must not be counted as angle brackets
	// in scan_for_closing_angle_bracket
	const d = {
		'<': (a, b) => a < b,
		'<=': (a, b) => a <= b,
		'>': (a, b) => a > b,
		'>=': (a, b) => a >= b
	};

	// Comparison operators in multiple arrow bodies
	const e = {
		lt: (a, b) => a < b,
		lte: (a, b) => a <= b,
		gt: (a, b) => a > b,
		gte: (a, b) => a >= b
	};

	// Complex expression inside brackets on comparison RHS
	// check_indexed_type_pattern must not assume type args for expressions
	if (x < a[b - 1]) {
	}
	if (x < a[b + 1]) {
	}
	if (x < a[b * 2]) {
	}

	// Bracket with expression in other contexts
	const f = i < a[n - 1];
	const g = a[i] < a[i - 1];

	// Chained bracket access with expression
	const h = x < a[b][c - 1];
</script>
