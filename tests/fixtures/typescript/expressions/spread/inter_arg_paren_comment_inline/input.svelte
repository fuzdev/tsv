<script>
	// a line comment left behind by a spread's stripped parens runs to end of line, so
	// the argument list breaks around it — flushed inline it would escape past the `)`
	// and the `;`
	fn(
		...b, // i
		c
	);

	new A(
		...b, // i
		c
	);

	obj.method(
		...b, // i
		c
	);

	// the same on an argument in the middle of the list
	fn(
		a,
		...b, // i
		c
	);

	// a same-line block rides the argument's line instead, so the list can stay collapsed
	fn(...b /* i */, c);
</script>
