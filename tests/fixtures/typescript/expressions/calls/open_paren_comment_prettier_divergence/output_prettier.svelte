<script>
	// line comment trailing `(` — single arg
	fn(
		// c1
		a,
	);

	// line comment trailing `(` — multiple args
	fn(
		// c2
		a,
		b,
	);

	// block comment trailing `(`, then own-line leading comment
	fn(
		/* paren */
		// lead
		a,
	);
</script>
