<script>
	// Object literal cast: the brace hugs the cast parens, ({ ... }) not (\n\t{.
	const a = /** @type {A} */ ({
		// comment
		width,
		height
	});

	// Array literal cast: the bracket hugs likewise, ([ ... ]).
	const b = /** @type {B} */ ([
		// comment
		one,
		two
	]);
</script>
