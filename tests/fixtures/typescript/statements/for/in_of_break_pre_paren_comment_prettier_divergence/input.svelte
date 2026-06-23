<script>
	// Pre-paren block comment preserved in the breaking (line-comment) layout
	for /* a */ (
		x // b
			of
			arr
	)
		expr;

	// A comment that merely contains the word `await` stays a comment — it must
	// not be promoted to a `for await` keyword (for-in can never be `for await`)
	for /* await */ (
		x // b
			in
			obj
	)
		expr;

	// Real `for await` is preserved in the breaking layout
	for await (
		const x // b
			of
			arr
	)
		expr;
</script>
