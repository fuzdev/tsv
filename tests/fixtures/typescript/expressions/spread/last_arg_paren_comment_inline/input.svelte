<script>
	// same-line comment left behind by a spread's stripped parens: it rides the argument's
	// line, and the line comment after the stripped `)` trails it
	fn(
		a,
		...b /* i */ // t
	);

	new A(
		a,
		...b /* i */ // t
	);

	obj.method(
		a,
		...b /* i */ // t
	);

	// a line comment inside the stripped parens runs to end of line, so the argument list
	// breaks around it — flushed inline, it would escape past the `)` and the `;`
	fn(
		a,
		...b // i
	);

	new A(
		...b // i
	);

	// same, with a block written after the `)`: it trails the argument ahead of the
	// deferred line comment
	obj.method(
		a,
		...b /* t */ // i
	);
	// two line comments inside the stripped parens: only the first can trail the
	// argument's line, the second takes its own — and a block written after the `)`
	// still lands ahead of both
	fn(
		a,
		...b /* t */ // i1
		// i2
	);
</script>
