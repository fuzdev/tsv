<script>
	// a line comment left behind by a spread's stripped parens runs to end of line, so a
	// line comment written after the `)` takes its own line
	fn(
		a,
		...b // i
		// t
	);

	new A(
		a,
		...b // i
		// t
	);

	obj.method(
		a,
		...b // i
		// t
	);
</script>
