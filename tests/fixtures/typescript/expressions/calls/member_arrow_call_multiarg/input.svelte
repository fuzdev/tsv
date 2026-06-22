<script>
	// Member expression call with multiple args, last is arrow returning call
	// Prettier hugs head args and breaks after =>
	something.method(firstArg, (x) =>
		anotherFunction(x, veryLongArgumentName, anotherVeryLongArgumentName, yetAnotherLongArg)
	);

	// Shorter case that fits on one line
	a.b(c, (x) => fn(x, y));

	// With destructuring
	a.b(c, ([d, e]) => process(d, e, moreArgs, evenMoreArgs, keepGoing));

	// Inner call breaks
	a.b(c, (x) =>
		fn(x, { veryLongPropertyName: someValue, anotherVeryLongProperty: anotherValue, third: t })
	);
</script>
