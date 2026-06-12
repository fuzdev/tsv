<script>
	/* short - stays inline */
	const short = [...other,1,2];
	/* long spread name - still inline (under 80) */
	const long = [...someLongIdentifierThatMakesThisExceedTheLineWidth,1,2];
	/* multiple spreads - stays inline */
	const multi = [...arr1,...arr2,...arr3,...arr4,...arr5];
	/* spread with numbers - one per line (spread is not a number) */
	const numbers = [...other,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25];
</script>
