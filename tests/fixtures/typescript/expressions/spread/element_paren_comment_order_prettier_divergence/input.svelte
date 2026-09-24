<script>
	// a spread's stripped parens leave an own-line comment behind; a comment written after
	// the `)` follows it, in the order the author wrote them — in an array literal and in
	// an object literal

	// the last element: a block after the `)` glues onto the inside comment's line, with or
	// without a trailing comma between them
	const a1 = [
		a,
		...b
		/* i */ /* t */
	];

	const o1 = {
		a,
		...b
		/* i */ /* t */
	};

	const a2 = [
		a,
		...b
		/* i */ /* t */
	];

	const o2 = {
		a,
		...b
		/* i */ /* t */
	};

	// a line comment inside the parens ends its line, so the comment after the `)` starts
	// the next one, before or after a trailing comma
	const a3 = [
		a,
		...b
		// i
		/* t */
	];

	const o3 = {
		a,
		...b
		// i
		/* t */
	};

	const a4 = [
		a,
		...b
		// i
		// t
	];

	const o4 = {
		a,
		...b
		// i
		// t
	};

	const a5 = [
		a,
		...b
		// i
		/* t */
	];

	const o5 = {
		a,
		...b
		// i
		/* t */
	};

	const a6 = [
		a,
		...b
		// i
		// t
	];

	const o6 = {
		a,
		...b
		// i
		// t
	};

	// a line comment on the element's line inside the parens ends that line too
	const a7 = [
		a,
		...b // i
		/* t */
	];

	const o7 = {
		a,
		...b // i
		/* t */
	};

	const a8 = [
		a,
		...b // i
		/* t */
	];

	const o8 = {
		a,
		...b // i
		/* t */
	};

	// the inside run may mix the two kinds
	const a9 = [
		a,
		...b
		// c
		/* i */ /* t */
	];

	const o9 = {
		a,
		...b
		// c
		/* i */ /* t */
	};

	// an element the list continues past: the comma moves ahead of both comments
	const a10 = [
		...b,
		/* i */
		/* t */ c
	];

	const o10 = {
		...b,
		/* i */
		/* t */ c
	};

	const a11 = [
		a,
		...b,
		// i
		/* t */ c
	];

	const o11 = {
		a,
		...b,
		// i
		/* t */ c
	};

	const a12 = [
		a,
		...b,
		// i
		// t
		c
	];

	const o12 = {
		a,
		...b,
		// i
		// t
		c
	};

	const a13 = [
		a,
		...b, // i
		/* t */ c
	];

	const o13 = {
		a,
		...b, // i
		/* t */ c
	};

	// a block written after the `)` leads the next element on its line, unless the author
	// ended its line right after it — after the comma, or on a line holding nothing else
	// but the `)` — where it keeps that line; either way it takes a line of its own above
	// an author blank, and a directive below it keeps its own line too
	const a14 = [
		a,
		...b,
		/* i */
		/* t */ c
	];

	const o14 = {
		a,
		...b,
		/* i */
		/* t */ c
	};

	const a15 = [
		a,
		...b,
		/* i */
		/* t */ c
	];

	const o15 = {
		a,
		...b,
		/* i */
		/* t */ c
	};

	const a16 = [
		a,
		...b,
		/* i */
		/* t */
		c
	];

	const o16 = {
		a,
		...b,
		// i
		/* t */
		c
	};

	const a17 = [
		a,
		...b,
		// i
		/* t */
		c
	];

	const o17 = {
		a,
		...b,
		/* i */
		/* t */
		c
	};

	const a18 = [
		a,
		...b,
		/* i */
		/* t */
		c
	];

	const o18 = {
		a,
		...b,
		// i
		/* t */
		c
	};

	const a19 = [
		a,
		...b,
		// i
		/* t */
		c
	];

	const o19 = {
		a,
		...b,
		/* i */
		/* t */
		c
	};

	const a20 = [
		a,
		...b,
		/* i */
		/* t */
		/* g */ c
	];

	const a21 = [
		a,
		...b,
		/* i */
		/* t1 */
		/* t2 */ c
	];

	const o20 = {
		a,
		...b,
		/* i */
		/* t */
		/* g */ c
	};

	const o21 = {
		a,
		...b,
		/* i */
		/* t1 */
		/* t2 */ c
	};

	const a22 = [
		a,
		...b, // i
		/* t */
		c
	];

	const a23 = [
		a,
		...b,
		/* i */
		/* t */
		// prettier-ignore
		c  +  d
	];

	const o23 = {
		a,
		...b,
		// i
		/* t */
		// prettier-ignore
		c: c  +  d
	};

	const a24 = [
		a,
		...b,
		// i
		/* t */

		c
	];

	const o24 = {
		a,
		...b,
		/* i */
		/* t */

		c
	};

	const a25 = [
		a,
		...b,
		/* i */
		/* t */

		c
	];

	// an author blank below a comma the author gave its own line is kept too
	const a26 = [
		a,
		...b, // i
		// j
		// t

		c
	];

	const o26 = {
		a,
		...b, // i
		// j
		// t

		c
	};

	// unchanged: an own-line block after the `)` of a line comment's share
	const a27 = [
		a,
		...b
		// i
		/* t */
	];

	const o27 = {
		a,
		...b
		// i
		/* t */
	};
</script>
