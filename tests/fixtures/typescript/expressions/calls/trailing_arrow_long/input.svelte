<script>
	// Exactly 100 chars at opening { - stays inline
	fn('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', (x) => {
		return y;
	});

	// 101 chars at opening { - breaks all args
	fn(
		'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
		(x) => {
			return y;
		}
	);

	// Multiple short args, 100 chars - stays inline
	fn('aaaaaaaaaaaaaaaaaaa', 'bbbbbbbbbbbbbbbbbbb', 'ccccccccccccccccccccccccccccccccccccc', (x) => {
		return y;
	});

	// Multiple short args, 101 chars - breaks all args
	fn(
		'aaaaaaaaaaaaaaaaaaa',
		'bbbbbbbbbbbbbbbbbbb',
		'cccccccccccccccccccccccccccccccccccccc',
		(x) => {
			return y;
		}
	);
</script>
