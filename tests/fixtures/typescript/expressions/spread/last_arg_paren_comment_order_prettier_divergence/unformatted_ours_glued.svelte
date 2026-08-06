<script>
	// a spread's stripped parens leave an own-line comment behind, and a block
	// written after the `)` trails the last argument too
	fn(
		a,
		...(b
		/* i */) /* t */
	);

	new A(
		a,
		...(b
		/* i */) /* t */
	);

	obj.method(
		a,
		...(b
		/* i */) /* t */
	);
</script>
