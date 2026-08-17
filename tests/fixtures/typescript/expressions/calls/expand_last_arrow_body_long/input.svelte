<script>
	// exactly 100 - the whole call fits, so the array body stays inline
	fn1(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, (x) => [c, d, e]);

	// 101 - the array body hugs the callee line and expands internally, with no added parens
	fn1(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, (x) => [
		c,
		d,
		e
	]);

	// 101 - an object body hugs the same way, keeping the parens the grammar requires
	fn1(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, (x) => ({
		f: 1,
		g: 2
	}));
</script>
