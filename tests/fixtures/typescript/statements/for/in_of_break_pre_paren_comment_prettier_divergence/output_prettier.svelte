<script>
	// Pre-paren block comment preserved in the breaking (line-comment) layout
	for (/* a */ x of arr) // b
		expr;

	// A comment that merely contains the word `await` stays a comment — it must
	// not be promoted to a `for await` keyword (for-in can never be `for await`)
	for (/* await */ x in obj) // b
		expr;

	// Real `for await` is preserved in the breaking layout
	for await (const x of arr) // b
		expr;
</script>
