<script>
	// line comment trailing `{` - single property
	const { // c1
		a
	} = o1;

	// line comment trailing `{` - multiple properties
	const { // c2
		b,
		c
	} = o2;

	// block comment trailing `{`, then own-line leading comment
	const { /* brace */
		// lead
		d
	} = o3;

	// an author blank BELOW the pulled comment survives — the blank is authorship, not
	// the container's leading gap
	const { // c3

		e
	} = o4;

	// the same, with the block spelling (an own-line comment is what pulls a block onto
	// the `{` line)
	const { /* brace2 */

		// lead
		f
	} = o5;

	// the blank ABOVE the comment is a different question and stays erased — it sits
	// against the delimiter, where both formatters drop it at every bracket
	const {
		// c4
		g
	} = o6;
</script>
