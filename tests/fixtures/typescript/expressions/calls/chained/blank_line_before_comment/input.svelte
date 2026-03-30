<script>
	// blank line before leading line comment
	const a = b
		.fn1()
		.fn2()

		// comment
		.fn3();

	// blank line before leading block comment
	const c = d
		.fn1()
		.fn2()

		/* comment */
		.fn3();

	// no blank line (control case)
	const e = f
		.fn1()
		.fn2()
		// comment
		.fn3();
</script>
