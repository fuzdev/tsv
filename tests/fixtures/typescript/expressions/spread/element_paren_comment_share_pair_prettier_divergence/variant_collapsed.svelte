<script>
	// two comments glued on one line inside a spread's stripped parens, on an element the
	// list continues past: each takes a line of its own, which keeps the list open
	fn(a, ...b, /* i */ /* j */ c);

	new A(a, ...b, /* i */ /* j */ c);

	const a1 = [a, ...b, /* i */ /* j */ c];

	const o1 = {
		a,
		...b,
		/* i */ /* j */
		c
	};

	// on the last element nothing follows it, so the pair keeps its line
	const a2 = [
		a,
		...b
		/* i */ /* j */
	];

	// a comment written after the `)` follows the pair, leading the next element
	fn(a, ...b /* t */, /* i */ /* j */ c);

	const a3 = [a, ...b /* t */, /* i */ /* j */ c];
</script>
