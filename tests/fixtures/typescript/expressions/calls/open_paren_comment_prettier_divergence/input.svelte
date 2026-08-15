<script>
	// line comment trailing `(` — single arg
	fn( // c1
		a
	);

	// line comment trailing `(` — multiple args
	fn( // c2
		a,
		b
	);

	// block comment trailing `(`, then own-line leading comment
	fn( /* paren */
		// lead
		a
	);

	// line comment trailing `(` — arguments the call force-expands (function
	// composition): the comment still stays on the `(` line
	fn( // c3
		arr.map((x) => x),
		b
	);

	// line comment trailing `(` — a last argument the expand-last layout would hug:
	// the comment still stays on the `(` line
	fn( // c4
		a,
		() => {
			b();
		}
	);
</script>
