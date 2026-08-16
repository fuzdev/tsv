<script lang="ts">
	// sole arrow argument, ternary body — the hug states reassemble the argument from its
	// signature and body, so the retained parens and their comment must survive it
	f((x) => (x ? a : b /* c */));

	// call body, and a call body reached through a trailing `!`
	f((x) => g() /* c */);
	f((x) => g()! /* c */);

	// `new` callee takes the same two arms
	new F((x) => (x ? a : b /* c */));
	new F((x) => g() /* c */);

	// member callee, and a chain long enough to force expansion
	obj.m((x) => (x ? a : b /* c */));
	obj.m((x) => g() /* c */);
	obj
		.a()
		.b()
		.m((x) => (x ? a : b /* c */));

	// `async` and a typed return are hug-eligible too
	f(async (x) => (x ? a : b /* c */));
	f((x): T => (x ? a : b /* c */));

	// a line comment forces the parens open, and the retained shell breaks the argument out
	f((x) =>
		x ? a : b // c
	);

	// last argument of a multi-argument call — the break state is the same reassembly
	f(1, (x) =>
		g() // c
	);

	// …and its twins: the `new` printer's multi-argument arm, the chain's, and the
	// object-body hug state, whose parens the layout synthesizes rather than retains
	new F(1, (x) => g() /* c */);
	obj
		.a()
		.b()
		.m(1, (x) => g() /* c */);
	f(1, (x) => ({ k: 1 }) /* c */);
	obj
		.a()
		.b()
		.m(1, (x) => ({ k: 1 }) /* c */);

	// a chain whose head call carries its own arguments reaches the forced-expansion
	// argument builder, a third layout family with its own reassembly
	f(expr, 1).g((x) => (x ? a : b /* c */));

	// the `new` printer's multi-argument arm keeps the head hug, where the plain call
	// above breaks the argument out under the same comment
	new F(1, (x) =>
		g() // c
	);
</script>
