<script>
	// Blank lines between statements in block body inside template ${}
	const a = `${(() => {
		const x = 1;

		const y = 2;

		return x + y;
	})()}`;

	// Blank lines in arrow callback inside template ${}
	const b = `${items
		.map((item) => {
			const a = fn(item);

			const b = fn(a);

			return b;
		})
		.join(', ')}`;

	// Blank lines before/after comments in block body inside ${}
	const c = `${(() => {
		const x = 1;

		// comment after blank line
		const y = 2;

		return x + y;
	})()}`;
</script>
