<script>
	// a same-line comment left behind by an array spread's stripped parens rides the
	// element's line — printed by the spread's own doc, never again by the array
	const a1 = [...b /* i */, c];

	// a line comment inside the stripped parens runs to end of line, so the array breaks
	const a2 = [
		...b, // i
		c
	];

	const a3 = [
		a,
		...b, // i
		c
	];

	// an own-line comment is the array's share instead: a sibling line ahead of the next
	// element
	const a4 = [
		...b,
		/* i */
		c
	];

	// a block written after the stripped `)` sits before the comma, so it trails the
	// element; the own-line comment keeps the line below
	const a5 = [
		...b /* t */,
		/* i */
		c
	];

	// an author blank line after the spread survives
	const a6 = [
		...b, // i

		c
	];
</script>
