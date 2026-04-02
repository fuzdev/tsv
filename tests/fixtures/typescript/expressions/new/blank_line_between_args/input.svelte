<script>
	// Blank line between new expression args (no comments)
	const a = new Foo(
		x,

		y,
	);

	// Blank line between new expression args with leading comments
	const b = new Foo(
		x,

		// comment
		y,
	);

	// Blank line with trailing line comment
	const c = new Foo(
		x, // trailing

		y,
	);

	// Mixed: some blank lines, some not
	const d = new Foo(
		x,
		y,

		z,
	);
</script>
