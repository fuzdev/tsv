<script>
	// an own-line comment between `=>` and an object body drops the closing paren to
	// its own line
	fn(() =>
		// c
		({ a: 1 })
	);

	new A(() =>
		// c
		({ a: 1 })
	);

	obj.fn1(1).fn2(() =>
		// c
		({ a: 1 })
	);

	// an array body behaves the same
	new A(() =>
		// c
		[a, b]
	);

	// so does an own-line block comment
	new A(() =>
		/* c */
		({ a: 1 })
	);

	// the same rule one argument over, where the expand-last layout keeps the head
	// arguments inline
	fn(a, () =>
		// c
		({ b: 1 })
	);

	new A(a, () =>
		// c
		({ b: 1 })
	);

	obj.fn1(1).fn2(a, () =>
		// c
		({ b: 1 })
	);

	// the rule is the gap's, not the body's — a block body takes it too
	fn(() =>
		// c
		{
			a();
		}
	);

	new A(a, () =>
		// c
		{
			a();
		}
	);

	obj.fn1(1).fn2(() =>
		// c
		{
			a();
		}
	);

	// a curried argument asks the gap of its innermost `=>`
	fn(() => () =>
		// c
		({ a: 1 })
	);

	new A(() => () =>
		// c
		({ a: 1 })
	);

	obj.fn1(1).fn2(a, () => () =>
		// c
		({ b: 1 })
	);

	// control: a glued comment keeps the body hugged to `=>`
	new A(() => /* c */ ({ a: 1 }));

	new A(a, () => /* c */ ({ b: 1 }));
</script>
