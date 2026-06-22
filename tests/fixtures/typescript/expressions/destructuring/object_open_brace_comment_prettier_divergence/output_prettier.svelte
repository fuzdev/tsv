<script>
	// line comment trailing `{` - single property
	const {
		// c1
		a
	} = o1;

	// line comment trailing `{` - multiple properties
	const {
		// c2
		b,
		c
	} = o2;

	// block comment trailing `{`, then own-line leading comment
	const {
		/* brace */
		// lead
		d
	} = o3;
</script>
