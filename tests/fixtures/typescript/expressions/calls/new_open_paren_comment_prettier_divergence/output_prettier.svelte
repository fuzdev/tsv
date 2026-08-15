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

	// line comment trailing `(` — a last argument the expand-last layout would hug:
	// the comment still stays on the `(` line
	new Foo(
		// c6
		a,
		() => {
			b();
		}
	);

	// the same with a collection last argument
	new Foo(
		// c7
		a,
		{ b: 2 }
	);
</script>
