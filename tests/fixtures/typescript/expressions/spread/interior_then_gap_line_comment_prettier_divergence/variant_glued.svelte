<script>
	// a line comment written after a spread's stripped `)` and before the comma trails
	// the element past that pure separator; the own-line comment the parens held is the
	// list's own share and keeps the line below
	fn(
		...b,
		/* i */ // c
		d
	);

	const o = {
		...b,
		/* i */ // c
		d
	};

	const a = [
		...b,
		/* i */ // c
		d
	];
</script>
