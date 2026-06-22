<script>
	// Arrow with call body: own-line line comment preserved in expansion
	const a = very_long_name
		.then((x) => x)
		.then((x) => x)
		.catch(
			// line comment
			(e) => handle(e, a, b, c, d)
		);

	// Arrow with call body: own-line block comment preserved in expansion
	const b2 = very_long_name
		.then((x) => x)
		.then((x) => x)
		.catch(
			/* block comment */
			(e) => handle(e, a, b, c, d)
		);

	// Arrow with array body: own-line comment preserved
	const c = very_long_name
		.then((x) => x)
		.then((x) => x)
		.map(
			// line comment
			(x) => [x.a, x.b, x.c, x.d, x.e, x.f, x.g, x.h]
		);

	// Plain object: own-line comment preserved
	const d2 = very_long_name
		.then((x) => x)
		.then((x) => x)
		.other(
			// line comment
			{ a: 1, b: 2, c: 3, d: 4, e: 5 }
		);
</script>
