<script>
	fn('a', /* inline */ 'b');

	fn(
		'arg1',
		/* block comment */
		'arg2',
	);

	// Multiple args with multiple comments
	fn('a', /* c1 */ 'b', /* c2 */ 'c');

	// Leading and inter-argument comments
	fn(
		/* leading comment */
		'arg1',
		/* inter-arg comment */
		'arg2',
	);
</script>
