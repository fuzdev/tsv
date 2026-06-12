<script>
	// Short strings (<=24 chars including quotes) - poorly breakable, break after =
	const someReallyVeryVeryExtremelyLongVariableNameThatWillDefinitelyNeedToBreak1 =someFunction('short');
	const someReallyVeryVeryExtremelyLongVariableNameThatWillDefinitelyNeedToBreak2 =someFunction('exactly-twentyfour!');

	// Long strings (>25 chars including quotes) - NOT poorly breakable, break inside call
	const someReallyVeryVeryExtremelyLongVariableNameThatWillDefinitelyNeedToBreak3 = someFunction('more than twenty-five chars');
	const someReallyVeryVeryExtremelyLongVariableNameThatWillDefinitelyNeedToBreak4 = someFunction('this is a very very long string argument that definitely exceeds the threshold');

	// Edge case: exactly 25 chars (23 chars + 2 quotes)
	const someReallyVeryVeryExtremelyLongVariableNameThatWillDefinitelyNeedToBreak5 =someFunction('12345678901234567890123');
	// Edge case: 26 chars (24 chars + 2 quotes)
	const someReallyVeryVeryExtremelyLongVariableNameThatWillDefinitelyNeedToBreak6 = someFunction('123456789012345678901234');
</script>
