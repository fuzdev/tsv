<script>
	// Single-arg: JSDoc before block-body arrow forces expansion
	a.b(
		/** @param {T} x */ (x) => {
			fn(x);
		}
	);

	// Single-arg: regular block comment before block-body arrow
	a.b(
		/* cast */ (x) => {
			fn(x);
		}
	);

	// Two-arg chain: JSDoc on second arg blocks expand-first
	a.b(
		(c, d) => {
			c.e = d;
			return c;
		},
		/** @type {Record<string, string[]>} */ {}
	);

	// Two-arg non-chain: same pattern without member expression
	fn(
		(c, d) => {
			c.e = d;
			return c;
		},
		/** @type {Record<string, string[]>} */ {}
	);

	// Two-arg new expression: same pattern
	new A(
		(c, d) => {
			c.e = d;
			return c;
		},
		/** @type {Record<string, string[]>} */ {}
	);

	// Control: no comment, block-body arrow stays hugged
	a.b((x) => {
		fn(x);
	});
</script>
