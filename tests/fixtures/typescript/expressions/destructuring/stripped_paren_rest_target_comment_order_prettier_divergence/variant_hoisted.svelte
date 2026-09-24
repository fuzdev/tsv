<script>
	// a rest element's stripped parens leave an own-line comment behind; a comment written
	// after the `)` follows it, in the order the author wrote them — in an array and in an
	// object assignment pattern

	// a block after the `)` glues onto the inside comment's line
	[
		...a /* t */
		/* i */
	] = x;

	({
		...a /* t */
		/* i */
	} = x);

	[
		b,
		...a /* t */
		/* i */
	] = x;

	({
		b,
		...a /* t */
		/* i */
	} = x);

	// a multi-line block inside the parens keeps its lines
	({
		...a /* t */
		/* i1
	i2 */
	} = x);

	// a line comment inside the parens ends its line, so the comment after the `)` starts
	// the next one
	[
		b,
		...a /* t */
		// i
	] = x;

	({
		b,
		...a /* t */
		// i
	} = x);

	[
		b,
		...a
		// i // t
	] = x;

	({
		b,
		...a
		// i // t
	} = x);

	// a line comment on the rest's line inside the parens ends that line too
	[
		...a /* t */ // i
	] = x;

	({
		...a /* t */ // i
	} = x);

	// a nested pattern, and a for-of head
	[
		b,
		[
			c,
			...a /* t */
			// i
		]
	] = x;

	for ([
		b,
		...a /* t */
		/* i */
	] of xs);

	// unchanged: an own-line block after the `)` of a line comment's share
	[
		b,
		...a
		// i
		/* t */
	] = x;

	({
		b,
		...a
		// i
		/* t */
	} = x);

	[
		b,
		...a // i
		/* t */
	] = x;
</script>
