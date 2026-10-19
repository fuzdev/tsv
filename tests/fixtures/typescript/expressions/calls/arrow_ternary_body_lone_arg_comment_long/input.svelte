<script lang="ts">
	// a block comment run after `=>` the author broke after keeps its own line when the lone
	// ternary-body arrow breaks out onto its own line
	fffffffffffffffffffffffffffffffffffffffffffff(
		(attttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
			/* c */
			aaaaaaaaaaaaaaaaaaaa ? bbbbbbbbbbbbbbbbbbbbbbbb : cccccccccccccccccccc
	);
	fffffffffffffffffffffffffffffffffffffffffffff(
		(attttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
			/* c */ /* d */
			aaaaaaaaaaaaaaaaaaaa ? bbbbbbbbbbbbbbbbbbbbbbbb : cccccccccccccccccccc
	);
	obj.fn(
		(attttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
			/* c */
			aaaaaaaaaaaaaaaaaaaa ? bbbbbbbbbbbbbbbbbbbbbbbb : cccccccccccccccccccc
	);
	obj.fn(
		(attttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
			/* c */ /* d */
			aaaaaaaaaaaaaaaaaaaa ? bbbbbbbbbbbbbbbbbbbbbbbb : cccccccccccccccccccc
	);
	// the parameters break, and the body with its comment fits on the `) =>` line
	z.a()
		.b()
		.fn(
			(
				attttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt
			) => (/* c */ aaaaaaaaaaaaaaaaaaaa ? bbbbbbbbbbbbbbbbbbbbbbbb : cccccccccccccccccccc)
		);
	z.a()
		.b()
		.fn(
			(
				attttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt
			) => (/* c */ /* d */ aaaaaaaaaaaaaaaaaaaa ? bbbbbbbbbbbbbbbbbbbbbbbb : cccccccccccccccccccc)
		);
	// the `=>` line at 100 chars - the comment holds the body below `=>`, so the parameters stay
	fn(
		(atttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
			/* c */
			a ? b : c
	);
	fn(
		(atttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
			/* c */ /* d */
			a ? b : c
	);
	fn(
		x,
		(atttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
			/* c */
			a ? b : c,
		y
	);
	fn(
		x,
		(atttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) =>
			/* c */ /* d */
			a ? b : c,
		y
	);
	// the `=>` line at 101 chars - the parameters break and the run joins the body in parens
	fn(
		(
			attttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt
		) => (/* c */ /* d */ a ? b : c)
	);
	fn(
		x,
		(
			attttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt
		) => (/* c */ /* d */ a ? b : c),
		y
	);
</script>
