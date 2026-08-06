<script>
	// a spread's stripped parens leave an own-line comment behind, and a block
	// written after the `)` trails the last argument too
	fn(
		a,
		...b /* t */
		/* i */
	);

	new A(
		a,
		...b /* t */
		/* i */
	);

	obj.method(
		a,
		...b /* t */
		/* i */
	);
</script>
