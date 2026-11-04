<script>
	// The init fits in exactly 100 columns: the trailing comment past it is not measured
	for (
		let aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb + ccccccccc; // c
		;
		i++
	) {}

	// At 101 columns the init breaks, and the comment trails its last line
	for (
		let aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa =
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb + cccccccccc; // c
		;
		i++
	) {}
</script>
