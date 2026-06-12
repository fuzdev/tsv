<script>
	// Template literals without expressions - short if raw length <= 25 and no newlines

	// Short: 5 chars raw
	const someReallyVeryVeryExtremelyLongVariableNameThatWillDefinitelyNeedToBreak1 =
		someFunction(`hello`);

	// Short: 25 chars raw (boundary)
	const someReallyVeryVeryExtremelyLongVariableNameThatWillDefinitelyNeedToBreak2 =
		someFunction(`1234567890123456789012345`);

	// Long: 26 chars raw (exceeds threshold)
	const someReallyVeryVeryExtremelyLongVariableNameThatWillDefinitelyNeedToBreak3 = someFunction(
		`12345678901234567890123456`,
	);

	// With expression: NOT short (has interpolation)
	const someReallyVeryVeryExtremelyLongVariableNameThatWillDefinitelyNeedToBreak4 = someFunction(
		`hello ${world}`,
	);

	// With newline: NOT short (even though only 3 chars)
	const someReallyVeryVeryExtremelyLongVariableNameThatWillDefinitelyNeedToBreak5 = someFunction(`a
b`);

	// Empty: short (0 chars)
	const someReallyVeryVeryExtremelyLongVariableNameThatWillDefinitelyNeedToBreak6 =
		someFunction(``);
</script>
