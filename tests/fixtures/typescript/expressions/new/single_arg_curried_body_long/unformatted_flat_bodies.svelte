<script>
	// object terminal at exactly 100 - stays inline
	new A((a) => (b) => ({ c: 1, d: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa }));

	// object terminal at 101 - hugs the callee line, the object expands internally
	new A((a) => (b) => ({ c: 1, d: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa }));

	// array terminal at 101 - hugs the same way, with no parens added around the array
	new A((a) => (b) => [c, d, aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa]);

	// control: the plain call hugs both terminal kinds identically
	fn1((a) => (b) => [c, d, aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa]);
</script>
