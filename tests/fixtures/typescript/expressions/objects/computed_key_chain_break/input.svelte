<script>
	// Both keys are "short" (ab=2 chars, [b]=4 chars, threshold=5)
	// Chain breaking should find optimal break point for both

	// Line at 100 chars - should NOT wrap
	let fits100 = {
		ab: aaaaaaaaaa.bbbbbbbbbb.cccccccccc.dddddddddd.eeeeeeeeee.ffffffffff.gggggggggg.hhhhhhhhhhhhhh,
		[b]: aaaaaaaaaa.bbbbbbbbbb.cccccccccc.dddddddddd.eeeeeeeeee.ffffffffff.gggggggggg.hhhhhhhhhhhhh,
	};

	// Line at 101 chars - should wrap at last segment
	let wraps101 = {
		ab: aaaaaaaaaa.bbbbbbbbbb.cccccccccc.dddddddddd.eeeeeeeeee.ffffffffff.gggggggggg
			.hhhhhhhhhhhhhhh,
		[b]: aaaaaaaaaa.bbbbbbbbbb.cccccccccc.dddddddddd.eeeeeeeeee.ffffffffff.gggggggggg
			.hhhhhhhhhhhhhh,
	};

	// Line at 103 chars with .ii - should break optimally at .ii, keeping max on first line
	let wraps103 = {
		ab: aaaaaaaaaa.bbbbbbbbbb.cccccccccc.dddddddddd.eeeeeeeeee.ffffffffff.gggggggggg.hhhhhhhhhhhhhhh
			.ii,
		[b]: aaaaaaaaaa.bbbbbbbbbb.cccccccccc.dddddddddd.eeeeeeeeee.ffffffffff.gggggggggg.hhhhhhhhhhhhhh
			.ii,
	};
</script>
