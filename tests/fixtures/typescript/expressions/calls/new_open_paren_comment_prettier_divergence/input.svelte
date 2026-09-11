<script>
	// line comment trailing `(` — single arg
	new Foo( // c1
		a
	);

	// line comment trailing `(` — multiple args
	new Foo( // c2
		a,
		b
	);

	// block comment trailing `(`, then own-line leading comment
	new Foo( /* paren */
		// lead
		a
	);

	// line comment trailing `(`, with a block comment trailing the last argument
	new Foo( // c3
		a,
		b /* t */
	);

	// line comment trailing `(`, with a line comment trailing the last argument
	new Foo( // c4
		a,
		b // t
	);

	// line comment trailing `(`, with a blank line between the arguments
	new Foo( // c5
		a,

		b
	);

	// line comment trailing `(` — a last argument the expand-last layout would hug:
	// the comment still stays on the `(` line
	new Foo( // c6
		a,
		() => {
			b();
		}
	);

	// the same with a collection last argument
	new Foo( // c7
		a,
		{ b: 2 }
	);

	// block comment trailing `(`, an own-line block comment, then a glued pair
	new Foo( /* paren */
		/* lead */
		/* c1 */ /* c2 */
		a
	);

	// line comment trailing `(`, then a glued pair — single arg and several
	new Foo( // c9
		/* c1 */ /* c2 */
		a
	);
	new Foo( // c10
		/* c1 */ /* c2 */
		a,
		b
	);

	// an author blank below an own-line block comment survives
	new Foo( /* paren */
		/* lead */

		a
	);

	// an author blank BELOW the pulled comment survives — the blank is authorship, not
	// the container's leading gap (the blank ABOVE one stays erased, against the delimiter)
	new Foo( // c8

		a
	);
</script>
