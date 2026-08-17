<script>
	// object terminal flat at exactly 100 - the whole call fits, so it stays inline
	new A(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, (a) => (b) => ({ c: 1, d: 2 }));

	// object terminal at 101 - hugs the callee line, the object expands internally
	new A(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, (a) => (b) => ({
		c: 1,
		d: 2
	}));

	// array terminal at 101 - hugs the same way, with no parens added around the array
	new A(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, (a) => (b) => [
		c,
		d,
		e
	]);

	// control: the plain call hugs a curried object terminal identically
	fn1(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, (a) => (b) => ({
		c: 1,
		d: 2
	}));
</script>
