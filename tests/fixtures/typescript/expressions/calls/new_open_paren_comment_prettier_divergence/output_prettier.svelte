<script>
	// line comment trailing `(` — single arg
	new Foo(
		// c1
		a
	);

	// line comment trailing `(` — multiple args
	new Foo(
		// c2
		a,
		b
	);

	// block comment trailing `(`, then own-line leading comment
	new Foo(
		/* paren */
		// lead
		a
	);

	// line comment trailing `(`, with a block comment trailing the last argument
	new Foo(
		// c3
		a,
		b /* t */
	);

	// line comment trailing `(`, with a line comment trailing the last argument
	new Foo(
		// c4
		a,
		b // t
	);

	// line comment trailing `(`, with a blank line between the arguments
	new Foo(
		// c5
		a,

		b
	);
</script>
