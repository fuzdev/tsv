<script>
	// An indentable block comment leading a NESTED conditional in the alternate: its
	// continuation lines take the branch's indent, ahead of the nested test
	const x1 = a
		? b
		: /* y
			 */ aaa
			? bbb
			: ccc;

	// JSDoc-style star lines land at the same column
	const x2 = a
		? b
		: /** y
			 * z
			 */ aaa
			? bbb
			: ccc;

	// A comment the author broke after: the nested test drops below it at that indent
	const x3 = a
		? b
		: /* y
			 */
			aaa
			? bbb
			: ccc;

	// Deeper in a chain, one level further
	const x4 = a
		? b
		: c
			? d
			: /* y
				 */ aaa
				? bbb
				: ccc;

	// In the consequent the comment breaks the conditional, so the nested one prints bare
	const x5 = a
		? /* y
			 */ aaa
			? bbb
			: ccc
		: c;

	// A preserved (non-indentable) block keeps its own columns and still breaks it
	const x6 = a
		? /* y
	z */ aaa
			? bbb
			: ccc
		: c;

	// Controls: a non-nested branch and the test position
	const x7 = a
		? b
		: /* y
			 */ ccc;
	const x8 =
		/* y
		 */ (aaa ? bbb : ccc) ? b : c;
</script>
