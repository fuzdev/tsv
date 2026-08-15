<script>
	// a block comment leading an argument survives the expand-all layout, on the
	// argument's line where the author wrote it
	fn(a, /* x
y */ (b), {});

	new A(a, /* x
y */ (b), {});

	obj.fn1(1).fn2(a, /* x
y */ (b), {});

	// the same comment written before the comma trails the argument it follows
	fn(a /* x
y */, (b), {});

	// a run of blocks trailing an argument keeps every one of them — the same-line
	// question is asked of each comment's own neighbour, not of the argument behind
	// the whole run
	fn(a /* x
y */ /* c */, (b), {});
</script>
