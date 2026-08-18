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

	// A comment the author GLUED to the operator trails it, and the value opens
	// below — the same answer the declarator and the class field give.
	a = // comment
		expr;

	a += // comment
		expr;

	const b = // comment
		expr;

	class C {
		p = // comment
			expr;
	}
</script>
