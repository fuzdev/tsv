<script>
	// object body at exactly 100 - stays inline
	new A((x) => ({ b: 1, c: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa }));

	// object body at 101 - hugs the callee line, the object expands internally
	new A((x) => ({ b: 1, c: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa }));

	// array body at 101 - hugs the same way, with no parens added around the array
	new A((x) => [b, c, aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa]);

	// control: the plain call hugs both body kinds identically
	fn1((x) => [b, c, aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa]);
</script>
