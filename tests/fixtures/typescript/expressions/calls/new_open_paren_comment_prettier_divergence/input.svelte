<script>
	// line comment trailing `(` — single arg
	new Foo( // c1
		a,
	);

	// line comment trailing `(` — multiple args
	new Foo( // c2
		a,
		b,
	);

	// block comment trailing `(`, then own-line leading comment
	new Foo( /* paren */
		// lead
		a,
	);
</script>
