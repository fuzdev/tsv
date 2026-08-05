<script>
	// two blocks past the last element's comma keep their run order
	const a = [1, 2, /* c1 */ /* c2 */];

	// blocks on both sides of the last element's comma join one run
	const b = [3, 4 /* c3 */, /* c4 */];

	// line comment past the last element's comma trails it
	const c = [5, 6, // c5
	];

	// block then line comment past the last element's comma
	const d = ['aaaa', 'bbbb', /* c6 */ // t
	];
</script>
