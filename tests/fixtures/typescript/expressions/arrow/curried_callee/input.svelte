<script>
	// A curried chain as the callee of a call or `new` opens its parens onto their own
	// lines whenever the terminal body cannot stay on the last head's line

	// A plain body hangs, so the parens open even though the heads fit together
	(
		(a) => (b) => (c) =>
			test
	)();

	// A hugging or block body keeps the last head's line, and a short chain stays inline
	((a) => (b) => ({ k: 1 }))();
	((a) => (b) => {
		return b;
	})();
	((a) => (b) => (test ? 1 : 2))();

	// A break-forcing head stacks the heads at one shared indent inside the parens
	(
		({ x }) =>
		(b) =>
		(c) =>
			test
	)();
	(
		({ x }) =>
		(b) => ({ k: 1 })
	)(1, 2);
	new (
		({ x }) =>
		(b) =>
			test
	)();

	// The callee of a member chain's first call takes the same shape, and an optional call's
	(
		({ x }) =>
		(b) =>
			test
	)().p.q();
	((a) => (b) => ({ k: 1 }))().p;
	(
		({ x }) =>
		(b) =>
			test
	)?.();

	// Boundary: the heads and the hugging body's opener fit at exactly 100
	((argument1) => (argument2) => (argument3) => (argument4) => (argument5) => (a1234567890123) => ({
		k: 1
	}))();

	// One char over (101): the parens open and every head takes its own line
	(
		(argument1) =>
		(argument2) =>
		(argument3) =>
		(argument4) =>
		(argument5) =>
		(a12345678901234) => ({
			k: 1
		})
	)();
</script>
