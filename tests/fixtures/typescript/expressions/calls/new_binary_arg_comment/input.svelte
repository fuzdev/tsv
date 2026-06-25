<script>
	// leading own-line comment forces expansion; the lone binary-chain arg keeps
	// its continuation indent
	const a = new Foo(
		// lead
		c1 +
			// cont
			c2 +
			c3
	);

	// inter-argument comment before a binary-chain last arg keeps continuation indent
	const b = new Foo(
		x,
		/* c */ c1 +
			// cont
			c2 +
			c3
	);

	// ternary arg whose test is a logical chain keeps the continuation indent
	const c = new Foo(
		x,
		/* c */ cond1 &&
			// cont
			cond2
			? p
			: q
	);

	// trailing line comment after a binary-chain arg keeps continuation indent
	const d = new Foo(
		c1 +
			// cont
			c2 +
			c3, // trail
		y
	);

	// assignment-expression arg keeps its clarity parens through the comment path
	const e = new Foo(/* c */ (a = b), c);

	// a blank line between args forces expansion; a binary-chain arg (broken by its
	// own internal comment) keeps its continuation indent on that path too
	const f = new Foo(
		a,

		c1 +
			// cont
			c2 +
			c3
	);
</script>
