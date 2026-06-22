<script>
	/* short - stays inline */
	const short = { ...other, a: 1 };
	/* at boundary - stays inline */
	const boundary = { ...someLongIdentifierThatMakesThisAlmostExceedOneHundredCharacterWidth, a: 1 };
	/* exceeds boundary - wraps */
	const long = {
		...someLongIdentifierThatDefinitelyMakesThisExceedTheOneHundredCharacterLineWidth,
		a: 1
	};
	/* spread with many properties - wraps */
	const mixed = {
		...other,
		prop1: 'value1',
		prop2: 'value2',
		prop3: 'value3',
		prop4: 'value4',
		prop5: 'value5'
	};
</script>
