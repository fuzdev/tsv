<script lang="ts">
	// an own-line comment between `=>` and the body keeps a lone arrow argument hugged to `new Fn(`
	// while the signature fits; past the print width the argument breaks out
	// 100 chars - hugged
	new Fn((attttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
		/* c */
		a ? b : c
	);
	// 101 chars - the argument takes its own line
	new Fn(
		(atttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
			/* c */
			a ? b : c
	);
	// a line comment
	new Fn((attttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
		// c
		a ? b : c
	);
	new Fn(
		(atttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
			// c
			a ? b : c
	);
	// a member callee
	new a.Fn(
		(atttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
			/* c */
			a ? b : c
	);
	// a declaration keeps the call on the `=` line
	const v = new Fn(
		(atttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
			/* c */
			a ? b : c
	);
	// a call and a binary body
	new Fn(
		(atttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
			/* c */
			g(a, b)
	);
	new Fn(
		(atttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
			/* c */
			a + b
	);
	// the same rule for a plain callee
	fn((attttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
		/* c */
		a ? b : c
	);
	fn(
		(atttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
			/* c */
			a ? b : c
	);
	// broken out, the `=>` line at 100 chars, then at 101 chars - the parameters break
	new Fn(
		(atttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
			/* c */
			a ? b : c
	);
	new Fn(
		(
			attttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt
		) =>
			/* c */
			a ? b : c
	);
	new Fn(
		(
			attttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt
		) =>
			// c
			a ? b : c
	);
	// a block comment before the arrow breaks the argument out, as for a plain callee
	new Fn(
		/* l */ (x) =>
			/* c */
			{
				go();
			}
	);
	new Fn(
		/* l */ (x) =>
			// c
			({ k: a })
	);
	fn(
		/* l */ (x) =>
			/* c */
			{
				go();
			}
	);
</script>
