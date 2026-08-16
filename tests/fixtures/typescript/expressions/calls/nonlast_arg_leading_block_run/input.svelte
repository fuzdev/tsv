<script>
	// a run of blocks leading a non-last argument keeps every comment: the glue
	// question is asked of each comment's own neighbour, not of the argument
	// behind the whole run
	fn(
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		/* c1 */ /* c2 */ bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
		{}
	);

	new A(
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		/* c1 */ /* c2 */ bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
		{}
	);

	obj
		.fn1(1)
		.fn2(
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
			/* c1 */ /* c2 */ bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
			{}
		);

	// the newline the author wrote after the run rides prettier's soft line, so
	// the run keeps the line it was given when the list breaks
	fn(
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		/* c1 */ /* c2 */
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
		{}
	);

	new A(
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		/* c1 */ /* c2 */
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
		{}
	);

	obj
		.fn1(1)
		.fn2(
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
			/* c1 */ /* c2 */
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
			{}
		);

	// the same run survives the expand-last hug, collapsed onto the call's line
	fn(a, /* c1 */ /* c2 */ b, () => {
		c();
	});

	new A(a, /* c1 */ /* c2 */ b, () => {
		c();
	});

	obj.fn1(1).fn2(a, /* c1 */ /* c2 */ b, () => {
		c();
	});
</script>
