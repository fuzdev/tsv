<script>
	/* short - stays inline */
	const short = {fn: () => 1, other: 2};
	/* at boundary - stays inline */
	const boundary = {fn: () => someLongIdentifierThatMakesThisAlmostExceedWidth, other: 2};
	/* exceeds boundary - wraps */
	const long = {
		fn: () => someLongIdentifierThatDefinitelyMakesThisExceedTheOneHundredCharacterLineWidth,
		other: 2,
	};
	/* multiple arrows - wraps */
	const multi = {
		fn1: () => value1,
		fn2: () => value2,
		fn3: () => value3,
		fn4: () => value4,
		fn5: () => value5,
		fn6: () => value6,
	};
</script>
