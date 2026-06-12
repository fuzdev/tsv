<script>
	// paren-line block comment, then own-line leading comment
	obj.method(
		/* paren */
		/* lead */
		arg,
	);

	// paren-line line comment, then own-line leading comment
	obj.method(
		// paren
		/* lead */
		arg,
	);

	// two-method chain, same gap shape
	obj.a().method(
		/* paren */
		/* lead */
		arg,
	);
</script>
