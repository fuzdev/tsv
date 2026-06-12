<script>
	// Assignment parens in function composition args (call with nested arrow/function)
	fn(
		(a = b),
		g(() => a),
	);
	fn(
		(a += b),
		g(() => a),
	);
	fn(
		(a &&= b),
		g(() => a),
	);
	fn(
		(a ||= b),
		g(() => a),
	);
	fn(
		(a ??= b),
		g(() => a),
	);

	// Multiple assignments with composition
	fn(
		(a = b),
		g(() => a),
		(c = d),
	);

	// Member expression LHS
	fn(
		(a[b] = c),
		g(() => a),
	);
	fn(
		(a.b = c),
		g(() => a),
	);

	// In return context
	function h() {
		return fn(
			(a = b),
			g(() => a),
			c,
			d,
		);
	}

	// Multiline content path (template with embedded newlines)
	fn(
		(a = b),
		`line1
line2`,
	);
</script>
