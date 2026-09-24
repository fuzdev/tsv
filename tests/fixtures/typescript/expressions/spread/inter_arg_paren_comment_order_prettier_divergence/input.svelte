<script>
	// a spread's stripped parens leave an own-line comment behind, on an argument the list
	// continues past; a comment written after the `)` follows it, in the order the author
	// wrote them, and the comma moves ahead of both

	// a block written after the `)`, before the comma: the comma moves ahead of both
	// comments, and the block leads the next argument below the inside one
	fn(
		...b,
		/* i */
		/* t */ c
	);

	fn(
		a,
		...b,
		/* i */
		/* t */ c
	);

	new A(
		a,
		...b,
		/* i */
		/* t */ c
	);

	obj.method(
		a,
		...b,
		/* i */
		/* t */ c
	);

	// a line comment inside the parens ends its line, so the comment after the `)` starts
	// the next one — a block before the comma, or a line comment after it
	fn(
		a,
		...b,
		// i
		/* t */ c
	);

	new A(
		a,
		...b,
		// i
		/* t */ c
	);

	obj.method(
		a,
		...b,
		// i
		/* t */ c
	);

	fn(
		a,
		...b,
		// i
		// t
		c
	);

	new A(
		a,
		...b,
		// i
		// t
		c
	);

	obj.method(
		a,
		...b,
		// i
		// t
		c
	);

	// a line comment on the argument's line inside the parens: the comma moves ahead of it,
	// and the block after the `)` leads the next argument
	fn(
		a,
		...b, // i
		/* t */ c
	);

	new A(
		a,
		...b, // i
		/* t */ c
	);

	obj.method(
		a,
		...b, // i
		/* t */ c
	);

	// a block written after the `)` leads the next argument on its line, unless the author
	// ended its line right after it — after the comma, or on a line holding nothing else
	// but the `)` — where it keeps that line; either way it takes a line of its own above
	// an author blank, and a directive below it keeps its own line too
	fn(
		a,
		...b,
		/* i */
		/* t */ c
	);

	new A(
		a,
		...b,
		/* i */
		/* t */ c
	);

	fn(
		a,
		...b,
		/* i */
		/* t */ c
	);

	new A(
		a,
		...b,
		/* i */
		/* t */ c
	);

	fn(
		a,
		...b,
		/* i */
		/* t */
		c
	);

	new A(
		a,
		...b,
		// i
		/* t */
		c
	);

	obj.method(
		a,
		...b,
		/* i */
		/* t */
		c
	);

	obj.method(
		a,
		...b,
		// i
		/* t */
		c
	);

	fn(
		a,
		...b,
		/* i */
		/* t */
		c
	);

	new A(
		a,
		...b,
		// i
		/* t */
		c
	);

	obj.method(
		a,
		...b,
		/* i */
		/* t */
		c
	);

	obj.method(
		a,
		...b,
		// i
		/* t */
		c
	);

	fn(
		a,
		...b,
		/* i */
		/* t */
		/* g */ c
	);

	fn(
		a,
		...b,
		/* i */
		/* t1 */
		/* t2 */ c
	);

	new A(
		a,
		...b,
		/* i */
		/* t */
		/* g */ c
	);

	new A(
		a,
		...b,
		/* i */
		/* t1 */
		/* t2 */ c
	);

	obj.method(
		a,
		...b,
		/* i */
		/* t */
		/* g */ c
	);

	obj.method(
		a,
		...b,
		/* i */
		/* t1 */
		/* t2 */ c
	);

	fn(
		a,
		...b, // i
		/* t */
		c
	);

	fn(
		a,
		...b,
		/* i */
		/* t */
		// prettier-ignore
		c  +  d
	);

	new A(
		a,
		...b,
		// i
		/* t */
		// prettier-ignore
		c  +  d
	);

	fn(
		a,
		...b,
		/* i */
		/* t */

		c
	);

	new A(
		a,
		...b, // i
		/* t */

		c
	);

	fn(
		a,
		...b,
		/* i */
		/* t */

		c
	);

	// an author blank below a comma the author gave its own line is kept too
	fn(
		a,
		...b, // i
		// j
		// t

		c
	);

	new A(
		a,
		...b, // i
		// j
		// t

		c
	);

	// unchanged: a comment after the comma, or on its own line below the `)`, leads the next
	// argument
	fn(
		a,
		...b,
		/* i */
		/* t */ c
	);

	fn(
		a,
		...b,
		// i
		/* t */ c
	);

	fn(
		a,
		...b, // i
		/* t */ c
	);

	fn(
		a,
		...b, // i
		/* t */ c
	);
</script>
