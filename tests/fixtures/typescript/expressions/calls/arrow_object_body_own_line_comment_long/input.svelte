<script>
	// an own-line comment between `=>` and the body keeps the argument hugged to `(` while
	// the signature fits; past the print width the argument breaks out instead of the
	// signature shattering in place

	// 100 chars — hugged
	fn(({ aaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbb, ccccccccccccccccccccccccccccc }) =>
		// c
		({ a: 1 })
	);

	// 101 chars — the argument takes its own line
	fn(
		({ aaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbb, cccccccccccccccccccccccccccccc }) =>
			// c
			({ a: 1 })
	);

	// the same rule in the `new`, member-chain and expand-last spellings
	new A(
		({ aaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbb, cccccccccccccccccccccccccccccc }) =>
			// c
			({ a: 1 })
	);

	obj.fn(
		({ aaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbb, cccccccccccccccccccccccccccccc }) =>
			// c
			({ a: 1 })
	);

	fn(
		a,
		({ aaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbb, cccccccccccccccccccccccccccccc }) =>
			// c
			({ b: 1 })
	);

	obj.fn(
		a,
		({ aaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbb, cccccccccccccccccccccccccccccc }) =>
			// c
			({ b: 1 })
	);

	// the rule is the gap's, not the body's — a block, call and ternary body take it too
	fn(
		({ aaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbb, cccccccccccccccccccccccccccccc }) =>
			// c
			{
				a();
			}
	);

	fn(
		({ aaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbb, cccccccccccccccccccccccccccccc }) =>
			// c
			g(a)
	);

	fn(
		({ aaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbb, cccccccccccccccccccccccccccccc }) =>
			// c
			a ? b : c
	);
</script>
