<script>
	// line comment trailing `(` — single arg
	obj.method(
		// c1
		a,
	);

	// line comment trailing `(` — multiple args
	obj.method(
		// c2
		a,
		b,
	);

	// block comment trailing `(`, then own-line leading comment
	obj.method(
		/* paren */
		// lead
		a,
	);

	// two-method chain, same gap shape
	obj.a().method(
		/* paren */
		// lead
		arg,
	);
</script>
