<script>
	// A single-line block trailing the params forces nothing, so the curried chain stays flat
	const fn1 = (x) => (y /* c */) => z;

	// Any arrow in the chain, at any depth
	const fn2 = (x) => (y /* c */) => (w) => z;
	const fn3 = (x) => (y) => (w /* c */) => z;

	// The outer arrow's own trailing param comment is not the trigger
	const fn4 = (x /* c */) => (y) => z;

	// The same chain as a call, member-chain and `new` argument
	fn((x) => (y /* c */) => z);
	a.b().c((x) => (y /* c */) => z);
	new Comp((x) => (y /* c */) => z);

	// A block-body callback keeps its hug — the same question, three more readers
	fn((y /* c */) => {
		call(y);
	});
	a.b().c((y /* c */) => {
		call(y);
	});
	new Comp((y /* c */) => {
		call(y);
	});

	// Control: a line comment DOES force the list multiline, and the call expands
	fn(
		(
			y // c
		) => {
			call(y);
		}
	);

	// Control: a multiline block forces it too
	fn(
		(
			y /* c
	c */
		) => {
			call(y);
		}
	);

	// Control: an own-line block forces it too
	fn(
		(
			y
			/* c */
		) => {
			call(y);
		}
	);

	// Control: no comment at all
	const fn5 = (x) => (y) => z;
	fn((y) => {
		call(y);
	});
</script>
