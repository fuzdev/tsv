<script>
	// a line comment written after a spread's stripped `)`, before or after the comma,
	// follows the own-line comment the parens held: the comma moves ahead of both, and the
	// line comment trails the inside comment's line
	fn(...(b
	/* i */
	)// c
	, d);

	new A(...(b
	/* i */
	)// c
	, d);

	const o = {...(b
	/* i */
	)// c
	, d};

	const a = [...(b
	/* i */
	)// c
	, d];
</script>
