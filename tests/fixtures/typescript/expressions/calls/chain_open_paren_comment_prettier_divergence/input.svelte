<script>
	// line comment trailing `(` — single arg
	obj.method( // c1
		a
	);

	// line comment trailing `(` — multiple args
	obj.method( // c2
		a,
		b
	);

	// block comment trailing `(`, then own-line leading comment
	obj.method( /* paren */
		// lead
		a
	);

	// two-method chain, same gap shape
	obj.a().method( /* paren */
		// lead
		arg
	);

	// line comment trailing `(` — a last argument the expand-last layout would hug:
	// the comment still stays on the `(` line
	obj.a().method( // c3
		a,
		() => {
			b();
		}
	);

	// an author blank BELOW the pulled comment survives — the blank is authorship, not
	// the container's leading gap (the blank ABOVE one stays erased, against the delimiter)
	obj.method( // c4

		a
	);
</script>
