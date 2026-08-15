<script>
	// an inline block comment between an argument and its comma rides with that
	// argument, so the expand-last hug survives
	fn(a /* c */, () => {
		b();
	});

	new A(a /* c */, () => {
		b();
	});

	obj.fn1(1).fn2(a /* c */, () => {
		b();
	});

	// the same comment in a non-last gap of a longer list
	new A(a /* c */, b, () => {
		c();
	});

	// a line comment in that gap needs its own line, so every argument breaks out
	new A(
		a,
		// c
		b,
		() => {
			c();
		}
	);
</script>
