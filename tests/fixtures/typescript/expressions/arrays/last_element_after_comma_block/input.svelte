<script>
	// numbers-only array
	const a = [1, 2 /* c1 */];

	// inline array
	const b = ['aaaa', 'bbbb' /* c2 */];

	// line-continuation string forces expansion
	const c = [
		'aaaa\
aaaa',
		'bbbb' /* c3 */
	];

	// line comment forces expansion
	const d = [
		'aaaa', // x
		'bbbb' /* c4 */
	];
</script>
