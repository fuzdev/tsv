<script>
	// An annotation leading the FIRST argument refuses the first-argument hug, exactly as
	// any same-width ordinary comment does — the arguments break out instead.

	// Plain call
	setTimeout(
		/* @__PURE__ */ () => {
			work();
		},
		100
	);

	setTimeout(
		/* comment12 */ () => {
			work();
		},
		100
	);

	// `new` expression
	const a = new C(
		/* @__PURE__ */ () => {
			work();
		},
		100
	);

	const b = new C(
		/* comment12 */ () => {
			work();
		},
		100
	);

	// Member-chain call
	obj.on(
		/* @__PURE__ */ () => {
			work();
		},
		100
	);

	obj.on(
		/* comment12 */ () => {
			work();
		},
		100
	);
</script>
