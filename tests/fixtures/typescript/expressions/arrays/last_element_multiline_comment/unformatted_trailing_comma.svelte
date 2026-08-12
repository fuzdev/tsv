<script>
	// a multiline block comment glued after the last element expands the array
	const a = ['aaaa', 'bbbb', /* c1
c2 */];

	// an own-line multiline block comment before the closer keeps its own line
	const b = [
		'cccc',
		'dddd',
		/* c3
c4 */
	];

	// a line comment after the multiline block joins the trailing run
	const c = ['eeee', 'ffff', /* c5
c6 */ // t
	];

	// two multiline blocks keep their run order
	const d = ['gggg', 'hhhh', /* c7
c8 */ /* c9
c10 */];

	// a spread as the last element takes the same trailing gap
	const e = ['iiii', ...rest, /* c11
c12 */];

	// an authored blank line above the own-line comment is preserved
	const f = [
		'jjjj',
		'kkkk',

		/* c13
c14 */
	];
</script>
