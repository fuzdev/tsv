<script>
	// block comment trailing the last property is preserved in place
	const obj1 = {
		a: 1,
		b: 2 /* comment */
	};

	// single property
	const obj2 = {
		a: 1 /* comment */
	};

	// a multiline block comment trails the last property the same way
	const obj3 = {
		a: 1,
		b: 2 /* c1
c2 */
	};
</script>
