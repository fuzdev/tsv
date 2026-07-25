<script>
	// A block comment glued after the multiline element keeps its place
	const a = [
		'multi\
line' /* c1 */,
		x
	];

	// Glued after an interior element
	const b = [
		'multi\
line',
		y /* c2 */,
		z
	];

	// Glued after the last element
	const c = [
		'multi\
line',
		p,
		q /* c3 */
	];
</script>
