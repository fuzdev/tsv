<script>
	// single own-line block comment
	const a = import(
		/* comment */
		expr
	);

	// multiple own-line block comments
	const b = import(
		/* comment1 */
		/* comment2 */
		expr
	);

	// multiline block comment
	const c = import(
		/*
		 * multi-line comment
		 */
		expr
	);

	// single inline block comment (stays inline)
	const d = import(/* comment */ expr);

	// multiple inline block comments (stays inline)
	const e = import(/* c1 */ /* c2 */ expr);
</script>
