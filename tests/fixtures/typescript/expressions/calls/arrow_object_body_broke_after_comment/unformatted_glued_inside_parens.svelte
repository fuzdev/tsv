<script>
	// A block run glued to `=>` that the author broke after drops the body below the
	// arrow, and the call's `)` goes with it, in every printer that hugs such a body:
	// sole argument, `new`, member chain, and their multi-argument spellings.

	const b1 = fn((item) => (/* c */
	{
		a: 1
	}));

	const b2 = fn(x, (item) => (/* c */
	{
		a: 1
	}));

	const b3 = new A((item) => (/* c */
	{
		a: 1
	}));

	const b4 = new A(x, (item) => (/* c */
	{
		a: 1
	}));

	const b5 = obj.fn1(1).fn2((item) => (/* c */
	{
		a: 1
	}));

	const b6 = obj.fn1(1).fn2(x, (item) => (/* c */
	{
		a: 1
	}));

	// Control: a run the author did NOT break after keeps the body hugged to `=>`,
	// and the `)` with it.
	const b7 = fn((item) => /* c */ ({
		a: 1
	}));

	// Control: the same broken-after run collapses when the body fits, and the `)`
	// hugs — the break is the body's, not the comment's.
	const b8 = fn((item) => (/* c */
	{ a: 1 }));
</script>
