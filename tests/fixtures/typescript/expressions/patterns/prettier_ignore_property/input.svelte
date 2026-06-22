<script>
	// prettier-ignore preserves the destructuring property's source verbatim
	const {
		// prettier-ignore
		a =           1
	} = obj1;

	// trailing line comment is still handled normally
	const {
		b,
		// prettier-ignore
		c =           1 // comment
	} = obj2;

	// inner comment + spacing inside the default are preserved
	const {
		// prettier-ignore
		d =           /* comment */          1
	} = obj3;

	// nested: only the inner property is ignored, outer formats normally
	const {
		e: {
			// prettier-ignore
			f = [1, 2,    3]
		}
	} = obj4;
</script>
