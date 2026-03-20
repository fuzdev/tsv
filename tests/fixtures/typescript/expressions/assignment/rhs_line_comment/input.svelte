<script>
	let a;

	// line comment before identifier
	a =
		// comment
		expr;

	// line comment before call expression
	a =
		// comment
		fn();

	// compound assignment with line comment
	a +=
		// comment
		expr;

	// nested in condition (corpus pattern: attributes.js)
	if (
		a ===
		(a =
			// comment
			b)
	) {
	}
</script>
