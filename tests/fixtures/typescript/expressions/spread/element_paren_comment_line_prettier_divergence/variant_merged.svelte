<script>
	// a line comment left behind by a spread's stripped parens runs to end of line, so a
	// line comment written after the `)` takes its own line — in an array literal and in
	// an object literal, at both ends of the list
	const a1 = [
		...b, // i // t
		c
	];

	const a2 = [
		c,
		...b // i // t
	];

	const o1 = {
		...b, // i // t
		c
	};

	const o2 = {
		c,
		...b // i // t
	};
</script>
