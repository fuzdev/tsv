<script>
	// a block after the comma stays on the comma line, in front of the line comment the
	// author wrote after it
	const a = [
		'aaaa', /* c1 */ // c2
		'bbbb'
	];

	// a run of blocks after the comma keeps its order
	const b = [
		'aaaa', /* c1 */ /* c2 */ // c3
		'bbbb'
	];

	// blocks on both sides of the comma each keep their side of it
	const c = [
		'aaaa' /* c0 */, /* c1 */ // c2
		'bbbb'
	];

	// numeric elements take the same placement
	const d = [
		0, /* c1 */ // c2
		1
	];

	// a block with no line comment after it leads the next element instead - nothing
	// defers, so it keeps the side of the comma the author chose
	const e = ['aaaa', /* c1 */ 'bbbb'];
</script>
