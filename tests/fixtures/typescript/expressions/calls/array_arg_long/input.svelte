<script>
	// Array as last arg
	assert.deepStrictEqual(
		linesCollection.getViewLinesIndentGuidesLongLongLongMethodName(-1, -1),
		[1],
	);
	assert.deepStrictEqual(
		linesCollection.getViewLinesIndentGuidesLongLongLongMethodNam(-1, -1),
		[1, 2],
	);

	// Array as first arg
	assert.deepStrictEqual(
		[1],
		linesCollection.getViewLinesIndentGuidesLongLongLongMethodName(-1, -1),
	);

	// Object mixed with array
	assert.deepStrictEqual(
		{a: 1, b: 2},
		linesCollection.getViewLinesIndentGuidesLongLongLongName(-1),
	);
</script>
