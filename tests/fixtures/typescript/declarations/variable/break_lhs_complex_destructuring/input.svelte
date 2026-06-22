<script>
	// Complex destructuring - >2 properties with defaults
	const {
		propOne,
		propTwo = 1,
		propThree = 2,
		propFour
	} = getSomeLongObjectNameThatExceedsThePrintWidth();

	// Complex destructuring - >2 properties with non-shorthand
	const {
		a: propertyA,
		b: propertyB,
		c: propertyC
	} = getAnotherLongObjectNameThatExceedsPrintWidth();

	// Not complex - only 2 properties
	const { x = 1, y = 2 } = getShortObject();

	// Not complex - 3 properties but all shorthand without defaults
	const { foo, bar, baz } = getObject();
</script>
