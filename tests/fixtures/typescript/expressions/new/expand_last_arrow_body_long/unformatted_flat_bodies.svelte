<script>
	// object body flat at exactly 100 - stays inline
	new A(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, (x) => ({ a: 1, b: 2, c: 3, d: 4, e: 5, f: 66 }));

	// object body at 101 - hugs the callee line, the object expands internally
	new A(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, (x) => ({ a: 1, b: 2, c: 3, d: 4, e: 5, f: 66 }));

	// array body past 100 - hugs the same way, with no parens added around the array
	new A(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, (x) => [aaa, bbb, ccc, ddd, eee, fff, ggg, hhh, iii]);

	// control: the plain call hugs an object body identically
	fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, (x) => ({ a: 1, b: 2, c: 3, d: 4, e: 5, f: 66 }));
</script>
